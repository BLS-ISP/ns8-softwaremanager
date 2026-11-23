use anyhow::{Context, Result};
use std::collections::HashSet;
use std::process::Stdio;
use std::time::Duration;
use swm_common::{JobResult, PendingOsUpdate};
use tokio::io::{AsyncBufReadExt, BufReader};
use tokio::process::Command;

pub const SOFTWAREUPDATE_BIN: &str = "/usr/sbin/softwareupdate";

/// Parses human-readable `softwareupdate -l` size strings (e.g. `745821KiB`, `182450K`, `1.5GiB`, `512MiB`) into bytes.
pub fn parse_softwareupdate_size_bytes(raw: &str) -> u64 {
    let trimmed = raw.trim();
    if trimmed.is_empty() {
        return 0;
    }

    let mut num_part = String::new();
    let mut unit_part = String::new();
    for ch in trimmed.chars() {
        if (ch.is_ascii_digit() || ch == '.') && unit_part.is_empty() {
            num_part.push(ch);
        } else if !ch.is_whitespace() {
            unit_part.push(ch);
        }
    }

    let Ok(val) = num_part.parse::<f64>() else {
        return 0;
    };
    if !val.is_finite() || val <= 0.0 {
        return 0;
    }

    let mult: f64 = match unit_part.to_ascii_uppercase().as_str() {
        "K" | "KB" | "KIB" => 1024.0,
        "M" | "MB" | "MIB" => 1024.0 * 1024.0,
        "G" | "GB" | "GIB" => 1024.0 * 1024.0 * 1024.0,
        "T" | "TB" | "TIB" => 1024.0 * 1024.0 * 1024.0 * 1024.0,
        _ => 1.0,
    };

    (val * mult).round() as u64
}

fn classify_macos_update(label: &str, title: &str, recommended: bool) -> (String, String) {
    let combined = format!("{} {}", label, title).to_ascii_lowercase();
    if combined.contains("security") || combined.contains("xprotect") || combined.contains("mrt") {
        ("security".to_string(), "critical".to_string())
    } else if combined.contains("macos") || combined.contains("safari") {
        ("security".to_string(), "important".to_string())
    } else if recommended {
        ("updates".to_string(), "important".to_string())
    } else {
        ("updates".to_string(), "moderate".to_string())
    }
}

fn is_known_detail_key(seg: &str) -> Option<(&str, &str)> {
    let (k, v) = seg.split_once(':')?;
    let key = k.trim();
    if matches!(
        key,
        "Title" | "Version" | "Size" | "Recommended" | "Action" | "Deferred"
    ) {
        Some((key, v.trim()))
    } else {
        None
    }
}

fn build_update_from_modern_detail(label: &str, detail_line: &str) -> PendingOsUpdate {
    let cleaned = detail_line.trim().trim_end_matches(',');
    let mut pairs: Vec<(String, String)> = Vec::new();

    for seg in cleaned.split(',') {
        let trimmed = seg.trim();
        if trimmed.is_empty() {
            continue;
        }
        if let Some((k, v)) = is_known_detail_key(trimmed) {
            pairs.push((k.to_string(), v.to_string()));
        } else if let Some(last) = pairs.last_mut() {
            last.1.push_str(", ");
            last.1.push_str(trimmed);
        }
    }

    let mut raw_title = String::new();
    let mut version = String::new();
    let mut size_bytes = 0u64;
    let mut recommended = false;
    let mut needs_reboot = false;

    for (k, v) in pairs {
        match k.as_str() {
            "Title" => raw_title = v,
            "Version" => version = v,
            "Size" => size_bytes = parse_softwareupdate_size_bytes(&v),
            "Recommended" => recommended = v.eq_ignore_ascii_case("yes"),
            "Action" => {
                let lower = v.to_ascii_lowercase();
                if lower.contains("restart") || lower.contains("shut down") {
                    needs_reboot = true;
                }
            }
            _ => {}
        }
    }

    if raw_title.is_empty() {
        raw_title = label.to_string();
    }
    let title = if !version.is_empty() && !raw_title.contains(&version) {
        format!("{raw_title} ({version})")
    } else {
        raw_title
    };

    let (category, severity) = classify_macos_update(label, &title, recommended);
    let reboot_behavior = if needs_reboot {
        "always_requires_reboot".to_string()
    } else {
        "never_reboots".to_string()
    };

    PendingOsUpdate {
        update_id: label.to_string(),
        title,
        severity,
        category,
        size_bytes,
        reboot_behavior,
    }
}

fn build_update_from_legacy_detail(label: &str, detail_line: &str) -> PendingOsUpdate {
    let mut working = detail_line.trim().to_string();
    let lower = working.to_ascii_lowercase();
    let needs_reboot = lower.contains("[restart]") || lower.contains("[shut down]");
    let recommended = lower.contains("[recommended]");

    // Strip bracketed suffixes like `[recommended]`, `[restart]`
    while let Some(open_idx) = working.rfind('[') {
        if working[open_idx..].contains(']') {
            working.truncate(open_idx);
            working = working.trim_end().to_string();
        } else {
            break;
        }
    }

    let (title_part, size_bytes) = if let Some((lhs, rhs)) = working.rsplit_once(',') {
        (lhs.trim(), parse_softwareupdate_size_bytes(rhs))
    } else {
        (working.as_str(), 0)
    };

    let title = if title_part.is_empty() {
        label.to_string()
    } else {
        title_part.to_string()
    };

    let (category, severity) = classify_macos_update(label, &title, recommended);
    let reboot_behavior = if needs_reboot {
        "always_requires_reboot".to_string()
    } else {
        "never_reboots".to_string()
    };

    PendingOsUpdate {
        update_id: label.to_string(),
        title,
        severity,
        category,
        size_bytes,
        reboot_behavior,
    }
}

/// Parses Apple `softwareupdate -l` output (both modern `* Label: ...` and legacy `* <id>` formats)
/// into `PendingOsUpdate` items.
pub fn parse_softwareupdate_list(output: &str) -> Vec<PendingOsUpdate> {
    let mut updates = Vec::new();
    let lines: Vec<&str> = output.lines().collect();
    let mut idx = 0usize;

    while idx < lines.len() {
        let trimmed = lines[idx].trim();
        if let Some(label_raw) = trimmed.strip_prefix("* Label:") {
            let label = label_raw.trim();
            if !label.is_empty() {
                let mut detail = "";
                if idx + 1 < lines.len() && !lines[idx + 1].trim().starts_with('*') {
                    detail = lines[idx + 1];
                    idx += 1;
                }
                updates.push(build_update_from_modern_detail(label, detail));
            }
        } else if let Some(legacy_raw) = trimmed.strip_prefix('*') {
            let label = legacy_raw.trim();
            if !label.is_empty() {
                let mut detail = "";
                if idx + 1 < lines.len() && !lines[idx + 1].trim().starts_with('*') {
                    detail = lines[idx + 1];
                    idx += 1;
                }
                if detail.trim_start().starts_with("Title:") {
                    updates.push(build_update_from_modern_detail(label, detail));
                } else {
                    updates.push(build_update_from_legacy_detail(label, detail));
                }
            }
        }
        idx += 1;
    }

    updates
}

/// Filters `updates` by excluding any entry whose `update_id` or `title` matches `excluded_ids` case-insensitively.
pub fn filter_excluded_macos_updates(
    updates: Vec<PendingOsUpdate>,
    excluded_ids: &[String],
) -> Vec<PendingOsUpdate> {
    if excluded_ids.is_empty() {
        return updates;
    }
    let excluded: HashSet<String> = excluded_ids
        .iter()
        .map(|s| s.trim().to_ascii_lowercase())
        .filter(|s| !s.is_empty())
        .collect();

    updates
        .into_iter()
        .filter(|u| {
            let id_lower = u.update_id.trim().to_ascii_lowercase();
            let title_lower = u.title.trim().to_ascii_lowercase();
            !excluded.contains(&id_lower) && !excluded.contains(&title_lower)
        })
        .collect()
}

/// Builds the exact `/usr/sbin/softwareupdate -i <labels...>` CLI argument list.
pub fn build_softwareupdate_install_args(labels: &[String]) -> Vec<String> {
    let mut args = vec!["-i".to_string()];
    for lbl in labels {
        let trimmed = lbl.trim();
        if !trimmed.is_empty() {
            args.push(trimmed.to_string());
        }
    }
    args
}

/// Detects whether `softwareupdate -i` stdout/stderr indicates that a system reboot is required.
pub fn softwareupdate_output_indicates_reboot(stdout: &str, stderr: &str) -> bool {
    let combined = format!("{}\n{}", stdout, stderr).to_ascii_lowercase();
    combined.contains("please restart")
        || combined.contains("requires a restart")
        || combined.contains("must restart")
        || combined.contains("action: restart")
        || combined.contains("[restart]")
        || combined.contains("shut down")
}

/// Scans pending Apple OS updates via `/usr/sbin/softwareupdate -l` with timeout and exclusion filtering.
pub async fn scan_macos_os_updates(excluded_ids: &[String]) -> Result<Vec<PendingOsUpdate>> {
    if let Ok(mock_json) = std::env::var("SWM_MOCK_MACOS_UPDATES_JSON") {
        let parsed: Vec<PendingOsUpdate> = serde_json::from_str(&mock_json)?;
        return Ok(filter_excluded_macos_updates(parsed, excluded_ids));
    }
    if let Ok(mock_raw) = std::env::var("SWM_MOCK_MACOS_UPDATES_OUTPUT") {
        let parsed = parse_softwareupdate_list(&mock_raw);
        return Ok(filter_excluded_macos_updates(parsed, excluded_ids));
    }

    let timeout_secs = std::env::var("SWM_SCAN_TIMEOUT_SECS")
        .ok()
        .and_then(|v| v.parse::<u64>().ok())
        .unwrap_or(120);

    let mut cmd = Command::new(SOFTWAREUPDATE_BIN);
    cmd.arg("-l").kill_on_drop(true);

    let out = tokio::time::timeout(Duration::from_secs(timeout_secs), cmd.output())
        .await
        .context("softwareupdate -l timed out")?
        .context("Failed executing /usr/sbin/softwareupdate -l")?;

    let combined = format!(
        "{}\n{}",
        String::from_utf8_lossy(&out.stdout),
        String::from_utf8_lossy(&out.stderr)
    );

    if !out.status.success() && !combined.to_ascii_lowercase().contains("no new software available")
    {
        anyhow::bail!(
            "softwareupdate -l exited with code {}: {}",
            out.status.code().unwrap_or(1),
            combined.trim()
        );
    }

    let parsed = parse_softwareupdate_list(&combined);
    Ok(filter_excluded_macos_updates(parsed, excluded_ids))
}

/// Installs approved Apple OS updates via `/usr/sbin/softwareupdate -i <labels...>` with timeout,
/// live line-by-line output capture, exclusion filtering, and `reboot_required` detection.
pub async fn install_macos_os_updates(
    requested_ids: &[String],
    excluded_ids: &[String],
) -> Result<JobResult> {
    let excluded_set: HashSet<String> = excluded_ids
        .iter()
        .map(|s| s.trim().to_ascii_lowercase())
        .filter(|s| !s.is_empty())
        .collect();

    let install_all = requested_ids.is_empty()
        || requested_ids
            .iter()
            .any(|id| id.trim().eq_ignore_ascii_case("all") || id.trim() == "*");

    let mut reboot_hint_from_scan = false;
    let target_labels: Vec<String> = if install_all {
        let scanned = scan_macos_os_updates(excluded_ids).await?;
        reboot_hint_from_scan = scanned
            .iter()
            .any(|u| u.reboot_behavior == "always_requires_reboot");
        scanned.into_iter().map(|u| u.update_id).collect()
    } else {
        // Also consult scan if available so reboot_behavior metadata is honored
        if let Ok(scanned) = scan_macos_os_updates(excluded_ids).await {
            let req_lower: HashSet<String> = requested_ids
                .iter()
                .map(|s| s.trim().to_ascii_lowercase())
                .collect();
            reboot_hint_from_scan = scanned.iter().any(|u| {
                req_lower.contains(&u.update_id.to_ascii_lowercase())
                    && u.reboot_behavior == "always_requires_reboot"
            });
        }
        requested_ids
            .iter()
            .map(|s| s.trim().to_string())
            .filter(|s| !s.is_empty() && !excluded_set.contains(&s.to_ascii_lowercase()))
            .collect()
    };

    if target_labels.is_empty() {
        return Ok(JobResult {
            status: "succeeded".to_string(),
            exit_code: 0,
            reboot_required: crate::inventory::reboot::detect_os_pending_reboot(),
            stdout: "No matching macOS updates to install.".to_string(),
            stderr: String::new(),
        });
    }

    let timeout_secs = std::env::var("SWM_INSTALL_TIMEOUT_SECS")
        .ok()
        .and_then(|v| v.parse::<u64>().ok())
        .unwrap_or(3600);

    let args = build_softwareupdate_install_args(&target_labels);
    let mut child = Command::new(SOFTWAREUPDATE_BIN)
        .args(&args)
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .kill_on_drop(true)
        .spawn()
        .context("Failed spawning /usr/sbin/softwareupdate -i")?;

    let stdout_pipe = child.stdout.take();
    let stderr_pipe = child.stderr.take();

    let out_task = tokio::spawn(async move {
        let mut collected = String::new();
        if let Some(pipe) = stdout_pipe {
            let mut lines = BufReader::new(pipe).lines();
            while let Ok(Some(line)) = lines.next_line().await {
                collected.push_str(&line);
                collected.push('\n');
            }
        }
        collected
    });

    let err_task = tokio::spawn(async move {
        let mut collected = String::new();
        if let Some(pipe) = stderr_pipe {
            let mut lines = BufReader::new(pipe).lines();
            while let Ok(Some(line)) = lines.next_line().await {
                collected.push_str(&line);
                collected.push('\n');
            }
        }
        collected
    });

    let status = match tokio::time::timeout(Duration::from_secs(timeout_secs), child.wait()).await {
        Ok(wait_res) => wait_res.context("Failed waiting for softwareupdate -i")?,
        Err(_) => {
            let _ = child.kill().await;
            return Ok(JobResult {
                status: "failed".to_string(),
                exit_code: 124,
                reboot_required: false,
                stdout: out_task.await.unwrap_or_default(),
                stderr: format!("softwareupdate -i timed out after {timeout_secs}s"),
            });
        }
    };

    let stdout = out_task.await.unwrap_or_default();
    let stderr = err_task.await.unwrap_or_default();
    let exit_code = status.code().unwrap_or(1);
    let succeeded = status.success();

    let reboot_required = succeeded
        && (reboot_hint_from_scan
            || softwareupdate_output_indicates_reboot(&stdout, &stderr)
            || crate::inventory::reboot::detect_os_pending_reboot());

    Ok(JobResult {
        status: if succeeded { "succeeded" } else { "failed" }.to_string(),
        exit_code,
        reboot_required,
        stdout,
        stderr,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_macos_update_parse_modern_and_legacy_softwareupdate_list() {
        let modern_sample = "\
Software Update Tool

Finding available software
Software Update found the following new or updated software:
* Label: Command Line Tools for Xcode-16.1
\tTitle: Command Line Tools for Xcode, Version: 16.1, Size: 745821KiB, Recommended: YES,
* Label: macOS Sequoia 15.1.1-24B91
\tTitle: macOS Sequoia 15.1.1, Version: 15.1.1, Size: 1482910KiB, Recommended: YES, Action: restart,
* Label: macOS Sonoma 14.7.1 (a)-23H222a
\tTitle: macOS Security Response 14.7.1 (a), Version: 14.7.1 (a), Size: 128500K, Recommended: YES, Action: restart
";
        let updates = parse_softwareupdate_list(modern_sample);
        assert_eq!(updates.len(), 3);

        assert_eq!(updates[0].update_id, "Command Line Tools for Xcode-16.1");
        assert_eq!(updates[0].title, "Command Line Tools for Xcode (16.1)");
        assert_eq!(updates[0].category, "updates");
        assert_eq!(updates[0].severity, "important");
        assert_eq!(updates[0].size_bytes, 745_821 * 1024);
        assert_eq!(updates[0].reboot_behavior, "never_reboots");

        assert_eq!(updates[1].update_id, "macOS Sequoia 15.1.1-24B91");
        assert_eq!(updates[1].category, "security");
        assert_eq!(updates[1].reboot_behavior, "always_requires_reboot");

        assert_eq!(updates[2].update_id, "macOS Sonoma 14.7.1 (a)-23H222a");
        assert_eq!(updates[2].severity, "critical");
        assert_eq!(updates[2].category, "security");
        assert_eq!(updates[2].reboot_behavior, "always_requires_reboot");

        // Exclusion filtering
        let filtered = filter_excluded_macos_updates(
            updates,
            &["Command Line Tools for Xcode-16.1".to_string()],
        );
        assert_eq!(filtered.len(), 2);
        assert_eq!(filtered[0].update_id, "macOS Sequoia 15.1.1-24B91");

        // Legacy format
        let legacy_sample = "\
Software Update found the following new or updated software:
   * Security Update 2023-001-10.15.7
\tSecurity Update 2023-001 (10.15.7), 1542100K [recommended] [restart]
   * Safari16.6CatalinaAuto-16.6
\tSafari (16.6), 84512K [recommended]
";
        let legacy = parse_softwareupdate_list(legacy_sample);
        assert_eq!(legacy.len(), 2);
        assert_eq!(legacy[0].update_id, "Security Update 2023-001-10.15.7");
        assert_eq!(legacy[0].category, "security");
        assert_eq!(legacy[0].severity, "critical");
        assert_eq!(legacy[0].size_bytes, 1_542_100 * 1024);
        assert_eq!(legacy[0].reboot_behavior, "always_requires_reboot");
        assert_eq!(legacy[1].update_id, "Safari16.6CatalinaAuto-16.6");
        assert_eq!(legacy[1].reboot_behavior, "never_reboots");

        assert!(parse_softwareupdate_list("No new software available.\n").is_empty());
    }

    #[test]
    fn test_macos_update_install_args_and_reboot_output_detection() {
        let args = build_softwareupdate_install_args(&[
            "macOS Sequoia 15.1.1-24B91".to_string(),
            "Safari18.1.1SonomaAuto-18.1.1".to_string(),
        ]);
        assert_eq!(
            args,
            vec![
                "-i",
                "macOS Sequoia 15.1.1-24B91",
                "Safari18.1.1SonomaAuto-18.1.1"
            ]
        );
        assert!(softwareupdate_output_indicates_reboot(
            "Installed macOS Sequoia 15.1.1",
            "Done. Please restart immediately."
        ));
        assert!(!softwareupdate_output_indicates_reboot(
            "Done with Command Line Tools.",
            ""
        ));
    }
}
