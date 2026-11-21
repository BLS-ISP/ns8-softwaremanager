use crate::patching::windows_wua::PatchExecutionOutcome;
use anyhow::{Context, Result};
use std::collections::HashSet;
use std::path::Path;
use std::time::Duration;
use swm_common::PendingOsUpdate;
use tokio::process::Command;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LinuxPkgManager {
    Apt,
    Dnf,
}

pub fn detect_linux_pkg_manager() -> Option<LinuxPkgManager> {
    if Path::new("/usr/bin/apt-get").exists() || Path::new("/bin/apt-get").exists() {
        Some(LinuxPkgManager::Apt)
    } else if Path::new("/usr/bin/dnf").exists() || Path::new("/bin/dnf").exists() {
        Some(LinuxPkgManager::Dnf)
    } else {
        None
    }
}

fn requires_kernel_or_core_reboot(pkg_name: &str) -> bool {
    let lower = pkg_name.to_ascii_lowercase();
    lower.starts_with("linux-image")
        || lower.starts_with("kernel")
        || lower == "libc6"
        || lower == "glibc"
        || lower == "systemd"
        || lower == "dbus"
        || lower.starts_with("microcode")
        || lower.starts_with("intel-microcode")
        || lower.starts_with("amd64-microcode")
}

/// Parses `apt-get -s upgrade` output (`Inst <pkg> [<old>] (<new> <origin> [<arch>])`).
pub fn parse_apt_simulated_upgrade(
    output: &str,
    excluded_updates: &[String],
) -> Vec<PendingOsUpdate> {
    let excluded: HashSet<String> = excluded_updates
        .iter()
        .map(|s| s.trim().to_ascii_lowercase())
        .collect();
    let mut updates = Vec::new();

    for line in output.lines() {
        let trimmed = line.trim();
        let Some(rest) = trimmed.strip_prefix("Inst ") else {
            continue;
        };
        let mut tokens = rest.split_whitespace();
        let Some(pkg) = tokens.next() else {
            continue;
        };
        if excluded.contains(&pkg.to_ascii_lowercase()) {
            continue;
        }

        // Extract new version & origin inside parentheses `(<new_ver> <origin ...>)`
        let paren_part = rest
            .find('(')
            .and_then(|start| rest[start + 1..].find(')').map(|end| &rest[start + 1..start + 1 + end]))
            .unwrap_or("");
        let new_ver = paren_part.split_whitespace().next().unwrap_or("latest");
        let lower_paren = paren_part.to_ascii_lowercase();
        let is_security = lower_paren.contains("security");

        let reboot_behavior = if requires_kernel_or_core_reboot(pkg) {
            "always_requires_reboot"
        } else {
            "never_reboots"
        };

        updates.push(PendingOsUpdate {
            update_id: pkg.to_string(),
            title: format!("{pkg} ({new_ver})"),
            severity: if is_security { "critical" } else { "moderate" }.to_string(),
            category: if is_security { "security" } else { "updates" }.to_string(),
            size_bytes: 0,
            reboot_behavior: reboot_behavior.to_string(),
        });
    }

    updates
}

/// Parses `dnf check-update -q` output along with optional `dnf updateinfo list security -q` output.
pub fn parse_dnf_check_update(
    check_output: &str,
    security_output: &str,
    excluded_updates: &[String],
) -> Vec<PendingOsUpdate> {
    let excluded: HashSet<String> = excluded_updates
        .iter()
        .map(|s| s.trim().to_ascii_lowercase())
        .collect();

    let mut security_pkgs = HashSet::new();
    for line in security_output.lines() {
        // Format: FEDORA-2026-1234  Important/Sec.  openssl-1:3.0.7-27.el9.x86_64
        let parts: Vec<&str> = line.split_whitespace().collect();
        if let Some(last) = parts.last() {
            // Extract base package name before version hyphen or arch
            let base = extract_rpm_pkg_base(last);
            if !base.is_empty() {
                security_pkgs.insert(base.to_ascii_lowercase());
            }
        }
    }

    let mut updates = Vec::new();
    for line in check_output.lines() {
        let trimmed = line.trim();
        if trimmed.is_empty()
            || trimmed.starts_with("Last metadata")
            || trimmed.starts_with("Obsoleting")
            || trimmed.starts_with("Security:")
        {
            continue;
        }
        let cols: Vec<&str> = trimmed.split_whitespace().collect();
        if cols.len() < 3 {
            continue;
        }
        // e.g. `openssl.x86_64   1:3.0.7-27.el9   baseos`
        let name_arch = cols[0];
        let version = cols[1];
        let repo = cols[2];
        let pkg_name = name_arch
            .rsplit_once('.')
            .map(|(n, _arch)| n)
            .unwrap_or(name_arch);

        if pkg_name.is_empty() || excluded.contains(&pkg_name.to_ascii_lowercase()) {
            continue;
        }

        let is_security = security_pkgs.contains(&pkg_name.to_ascii_lowercase())
            || repo.to_ascii_lowercase().contains("security");

        let reboot_behavior = if requires_kernel_or_core_reboot(pkg_name) {
            "always_requires_reboot"
        } else {
            "may_require_reboot"
        };

        updates.push(PendingOsUpdate {
            update_id: pkg_name.to_string(),
            title: format!("{pkg_name} {version} ({repo})"),
            severity: if is_security { "important" } else { "moderate" }.to_string(),
            category: if is_security { "security" } else { "updates" }.to_string(),
            size_bytes: 0,
            reboot_behavior: reboot_behavior.to_string(),
        });
    }
    updates
}

fn extract_rpm_pkg_base(nevra: &str) -> &str {
    // Strip trailing .x86_64 / .noarch, then strip -release and -version
    let without_arch = nevra.rsplit_once('.').map(|(s, _)| s).unwrap_or(nevra);
    let without_rel = without_arch
        .rsplit_once('-')
        .map(|(s, _)| s)
        .unwrap_or(without_arch);
    without_rel
        .rsplit_once('-')
        .map(|(s, _)| s)
        .unwrap_or(without_rel)
}

/// Builds the exact non-interactive APT upgrade command arguments (`DEBIAN_FRONTEND=noninteractive`).
/// Always passes `-o Dpkg::Options::=--force-confdef` and `-o Dpkg::Options::=--force-confold`.
pub fn build_apt_upgrade_args(target_packages: &[String]) -> Vec<String> {
    let mut args = vec![
        "-y".to_string(),
        "-o".to_string(),
        "Dpkg::Options::=--force-confdef".to_string(),
        "-o".to_string(),
        "Dpkg::Options::=--force-confold".to_string(),
    ];

    if target_packages.is_empty() {
        args.insert(0, "upgrade".to_string());
    } else {
        args.insert(0, "install".to_string());
        args.insert(1, "--only-upgrade".to_string());
        for pkg in target_packages {
            args.push(pkg.clone());
        }
    }
    args
}

/// Builds the exact non-interactive DNF upgrade command arguments.
pub fn build_dnf_upgrade_args(
    approved_ids: &[String],
    excluded_updates: &[String],
    security_only: bool,
) -> Vec<String> {
    let mut args = vec!["upgrade".to_string(), "-y".to_string()];
    if security_only {
        args.push("--security".to_string());
    }
    for ex in excluded_updates {
        if !ex.trim().is_empty() {
            args.push(format!("--exclude={}", ex.trim()));
        }
    }
    let install_all = approved_ids.is_empty()
        || approved_ids
            .iter()
            .any(|id| id.eq_ignore_ascii_case("all") || id == "*");
    if !install_all {
        for pkg in approved_ids {
            if !pkg.trim().is_empty() {
                args.push(pkg.trim().to_string());
            }
        }
    }
    args
}

pub async fn scan_linux_os_updates(excluded_updates: &[String]) -> Result<Vec<PendingOsUpdate>> {
    if let Ok(mock_json) = std::env::var("SWM_MOCK_LINUX_UPDATES_JSON") {
        let items: Vec<PendingOsUpdate> = serde_json::from_str(&mock_json)?;
        let excluded: HashSet<String> = excluded_updates
            .iter()
            .map(|s| s.to_ascii_lowercase())
            .collect();
        return Ok(items
            .into_iter()
            .filter(|u| !excluded.contains(&u.update_id.to_ascii_lowercase()))
            .collect());
    }

    let Some(mgr) = detect_linux_pkg_manager() else {
        return Ok(Vec::new());
    };

    match mgr {
        LinuxPkgManager::Apt => {
            let out = tokio::time::timeout(
                Duration::from_secs(30),
                Command::new("apt-get")
                    .env("DEBIAN_FRONTEND", "noninteractive")
                    .args(["-s", "upgrade"])
                    .output(),
            )
            .await
            .context("apt-get -s upgrade timed out")?
            .context("Failed executing apt-get -s upgrade")?;

            let stdout = String::from_utf8_lossy(&out.stdout);
            Ok(parse_apt_simulated_upgrade(&stdout, excluded_updates))
        }
        LinuxPkgManager::Dnf => {
            let check = tokio::time::timeout(
                Duration::from_secs(45),
                Command::new("dnf").args(["check-update", "-q"]).output(),
            )
            .await
            .context("dnf check-update timed out")?
            .context("Failed executing dnf check-update")?;

            // dnf check-update returns 100 when updates are available, 0 when none!
            let code = check.status.code().unwrap_or(1);
            if code != 0 && code != 100 {
                anyhow::bail!(
                    "dnf check-update failed with exit code {code}: {}",
                    String::from_utf8_lossy(&check.stderr)
                );
            }
            if code == 0 {
                return Ok(Vec::new());
            }

            let sec_out = Command::new("dnf")
                .args(["updateinfo", "list", "security", "-q"])
                .output()
                .await
                .map(|o| String::from_utf8_lossy(&o.stdout).to_string())
                .unwrap_or_default();

            let stdout = String::from_utf8_lossy(&check.stdout);
            Ok(parse_dnf_check_update(&stdout, &sec_out, excluded_updates))
        }
    }
}

pub async fn install_linux_os_updates(
    approved_ids: &[String],
    excluded_updates: &[String],
    security_only: bool,
) -> Result<PatchExecutionOutcome> {
    let Some(mgr) = detect_linux_pkg_manager() else {
        anyhow::bail!("No supported Linux package manager (apt-get or dnf) found");
    };

    match mgr {
        LinuxPkgManager::Apt => {
            let _ = Command::new("apt-get")
                .env("DEBIAN_FRONTEND", "noninteractive")
                .args(["update", "-q"])
                .output()
                .await;

            let install_all = approved_ids.is_empty()
                || approved_ids
                    .iter()
                    .any(|id| id.eq_ignore_ascii_case("all") || id == "*");

            let target_packages: Vec<String> = if security_only || !excluded_updates.is_empty() || !install_all {
                let simulated = scan_linux_os_updates(excluded_updates).await?;
                let approved_set: HashSet<String> = approved_ids
                    .iter()
                    .map(|s| s.to_ascii_lowercase())
                    .collect();
                let filtered: Vec<String> = simulated
                    .into_iter()
                    .filter(|u| !security_only || u.category == "security")
                    .filter(|u| install_all || approved_set.contains(&u.update_id.to_ascii_lowercase()))
                    .map(|u| u.update_id)
                    .collect();

                if filtered.is_empty() {
                    return Ok(PatchExecutionOutcome {
                        succeeded: true,
                        exit_code: 0,
                        reboot_required: crate::inventory::reboot::detect_os_pending_reboot(),
                        installed_count: 0,
                        stdout: "No matching APT updates to install".to_string(),
                        stderr: String::new(),
                    });
                }
                filtered
            } else {
                Vec::new()
            };

            let count_hint = target_packages.len();
            let args = build_apt_upgrade_args(&target_packages);
            let out = Command::new("apt-get")
                .env("DEBIAN_FRONTEND", "noninteractive")
                .args(&args)
                .output()
                .await
                .context("Failed executing apt-get upgrade")?;

            let reboot_required = crate::inventory::reboot::detect_os_pending_reboot()
                || target_packages.iter().any(|p| requires_kernel_or_core_reboot(p));
            let succeeded = out.status.success();
            Ok(PatchExecutionOutcome {
                succeeded,
                exit_code: out.status.code().unwrap_or(1),
                reboot_required,
                installed_count: count_hint,
                stdout: String::from_utf8_lossy(&out.stdout).to_string(),
                stderr: String::from_utf8_lossy(&out.stderr).to_string(),
            })
        }
        LinuxPkgManager::Dnf => {
            let args = build_dnf_upgrade_args(approved_ids, excluded_updates, security_only);
            let out = Command::new("dnf")
                .args(&args)
                .output()
                .await
                .context("Failed executing dnf upgrade")?;

            let stdout = String::from_utf8_lossy(&out.stdout).to_string();
            let stderr = String::from_utf8_lossy(&out.stderr).to_string();
            let reboot_required = crate::inventory::reboot::detect_os_pending_reboot()
                || stdout.contains("kernel-")
                || stdout.contains("glibc-");
            let succeeded = out.status.success();
            Ok(PatchExecutionOutcome {
                succeeded,
                exit_code: out.status.code().unwrap_or(1),
                reboot_required,
                installed_count: approved_ids.len(),
                stdout,
                stderr,
            })
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_linux_patch_apt_parsing_and_noninteractive_flags() {
        let sim_out = "\
Reading package lists...
Building dependency tree...
Inst openssl [3.0.2-0ubuntu1.15] (3.0.2-0ubuntu1.18 Ubuntu:22.04/jammy-security [amd64])
Inst linux-image-6.8.0-45-generic [6.8.0-44.44] (6.8.0-45.45 Ubuntu:24.04/noble-security [amd64])
Inst htop [3.2.2-1] (3.2.2-2 Ubuntu:24.04/noble-updates [amd64])
";
        let updates = parse_apt_simulated_upgrade(sim_out, &["htop".to_string()]);
        assert_eq!(updates.len(), 2);
        assert_eq!(updates[0].update_id, "openssl");
        assert_eq!(updates[0].category, "security");
        assert_eq!(updates[0].severity, "critical");
        assert_eq!(updates[1].update_id, "linux-image-6.8.0-45-generic");
        assert_eq!(updates[1].reboot_behavior, "always_requires_reboot");

        let args = build_apt_upgrade_args(&["openssl".to_string()]);
        assert_eq!(
            args,
            vec![
                "install",
                "--only-upgrade",
                "-y",
                "-o",
                "Dpkg::Options::=--force-confdef",
                "-o",
                "Dpkg::Options::=--force-confold",
                "openssl"
            ]
        );

        let full_args = build_apt_upgrade_args(&[]);
        assert_eq!(full_args[0], "upgrade");
        assert!(full_args.contains(&"Dpkg::Options::=--force-confdef".to_string()));
        assert!(full_args.contains(&"Dpkg::Options::=--force-confold".to_string()));
    }

    #[test]
    fn test_linux_patch_dnf_parsing_and_security_args() {
        let check_out = "\
openssl.x86_64                1:3.0.7-27.el9_4               baseos
kernel.x86_64                 5.14.0-427.37.1.el9_4          baseos
vim-enhanced.x86_64           2:8.2.2637-20.el9              appstream
";
        let sec_out = "\
RLSA-2024:7001  Important/Sec.  openssl-1:3.0.7-27.el9_4.x86_64
";
        let updates = parse_dnf_check_update(check_out, sec_out, &["vim-enhanced".to_string()]);
        assert_eq!(updates.len(), 2);
        assert_eq!(updates[0].update_id, "openssl");
        assert_eq!(updates[0].category, "security");
        assert_eq!(updates[1].update_id, "kernel");
        assert_eq!(updates[1].reboot_behavior, "always_requires_reboot");

        let sec_args = build_dnf_upgrade_args(&[], &["broken-pkg".to_string()], true);
        assert_eq!(
            sec_args,
            vec!["upgrade", "-y", "--security", "--exclude=broken-pkg"]
        );
    }
}
