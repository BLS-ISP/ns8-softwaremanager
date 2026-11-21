use std::collections::HashMap;
use std::fs;
use std::path::Path;
use swm_common::{version_satisfies, DetectionRule, SoftwareEntry};
use sysinfo::System;

/// Checks whether any process in `blocked_processes` is currently running.
/// Matches both `firefox` and `firefox.exe` case-insensitively.
pub fn find_running_blocked_processes_in_list(
    blocked_processes: &[String],
    running_process_names: &[String],
) -> Vec<String> {
    if blocked_processes.is_empty() {
        return Vec::new();
    }

    let normalize = |name: &str| -> String {
        name.trim()
            .trim_end_matches(".exe")
            .trim_end_matches(".EXE")
            .to_ascii_lowercase()
    };

    let running_norm: Vec<String> = running_process_names.iter().map(|p| normalize(p)).collect();
    let mut active_blockers = Vec::new();

    for blocked in blocked_processes {
        let b_norm = normalize(blocked);
        if b_norm.is_empty() {
            continue;
        }
        if running_norm.iter().any(|r| r == &b_norm) {
            active_blockers.push(blocked.clone());
        }
    }

    active_blockers
}

pub fn check_blocked_processes(blocked_processes: &[String]) -> Vec<String> {
    if blocked_processes.is_empty() {
        return Vec::new();
    }

    if let Ok(mock_running) = std::env::var("SWM_MOCK_RUNNING_PROCESSES") {
        let names: Vec<String> = mock_running
            .split(',')
            .map(|s| s.trim().to_string())
            .filter(|s| !s.is_empty())
            .collect();
        return find_running_blocked_processes_in_list(blocked_processes, &names);
    }

    let mut sys = System::new();
    sys.refresh_processes();
    let names: Vec<String> = sys
        .processes()
        .values()
        .map(|p| p.name().to_string())
        .collect();
    find_running_blocked_processes_in_list(blocked_processes, &names)
}

fn read_file_version_string(path: &Path) -> Option<String> {
    let ext = path
        .extension()
        .and_then(|e| e.to_str())
        .unwrap_or("")
        .to_ascii_lowercase();

    #[cfg(windows)]
    if matches!(ext.as_str(), "exe" | "dll" | "msi") {
        if let Ok(out) = std::process::Command::new("powershell.exe")
            .args([
                "-NoProfile",
                "-NonInteractive",
                "-Command",
                &format!(
                    "(Get-Item -LiteralPath '{}' -ErrorAction SilentlyContinue).VersionInfo.ProductVersion",
                    path.to_string_lossy().replace('\'', "''")
                ),
            ])
            .output()
        {
            if out.status.success() {
                let ver = String::from_utf8_lossy(&out.stdout).trim().to_string();
                if !ver.is_empty() {
                    return Some(ver);
                }
            }
        }
    }

    // Fallback for plain text / marker files (e.g. `.version`, `.txt`, `.json`)
    if let Ok(text) = fs::read_to_string(path) {
        let first_line = text.lines().next().unwrap_or("").trim();
        if !first_line.is_empty() && first_line.len() <= 64 {
            return Some(first_line.to_string());
        }
    }

    None
}

#[cfg(windows)]
fn read_windows_registry_value(full_key: &str, value_name: Option<&str>) -> Option<String> {
    use winreg::enums::{HKEY_CURRENT_USER, HKEY_LOCAL_MACHINE, KEY_READ};
    use winreg::RegKey;

    let normalized = full_key.trim().replace('/', r"\");
    let (hive, subpath) = if let Some(rest) = normalized
        .strip_prefix(r"HKLM\")
        .or_else(|| normalized.strip_prefix(r"HKEY_LOCAL_MACHINE\"))
    {
        (HKEY_LOCAL_MACHINE, rest)
    } else if let Some(rest) = normalized
        .strip_prefix(r"HKCU\")
        .or_else(|| normalized.strip_prefix(r"HKEY_CURRENT_USER\"))
    {
        (HKEY_CURRENT_USER, rest)
    } else {
        (HKEY_LOCAL_MACHINE, normalized.as_str())
    };

    let root = RegKey::predef(hive);
    let subkey = root.open_subkey_with_flags(subpath, KEY_READ).ok()?;

    let Some(val_name) = value_name.filter(|v| !v.trim().is_empty()) else {
        return Some("exists".to_string());
    };

    if let Ok(s) = subkey.get_value::<String, _>(val_name) {
        return Some(s);
    }
    if let Ok(n) = subkey.get_value::<u32, _>(val_name) {
        return Some(n.to_string());
    }
    None
}

/// Evaluates a `DetectionRule` against the filesystem, registry (or mock registry map), and installed software inventory.
pub fn evaluate_detection_rule_with(
    rule: &DetectionRule,
    installed_software: &[SoftwareEntry],
    mock_registry: Option<&HashMap<String, String>>,
) -> bool {
    if rule.is_empty() {
        return false;
    }

    let op = rule.operator.as_deref().unwrap_or("gte");
    let expected_ver = rule.version.as_deref().map(str::trim).filter(|s| !s.is_empty());

    match rule.rule_type.trim().to_ascii_lowercase().as_str() {
        "file" => {
            let Some(path_str) = rule.path.as_deref().filter(|p| !p.trim().is_empty()) else {
                return false;
            };
            let path = Path::new(path_str);
            if !path.exists() {
                return false;
            }
            let Some(expected) = expected_ver else {
                return true;
            };
            if op.eq_ignore_ascii_case("exists") {
                return true;
            }
            match read_file_version_string(path) {
                Some(actual) => version_satisfies(&actual, expected, op),
                None => false,
            }
        }
        "registry" => {
            let key_path = rule
                .registry_key
                .as_deref()
                .or(rule.path.as_deref())
                .unwrap_or("")
                .trim();
            if key_path.is_empty() {
                return false;
            }

            let val_opt = if let Some(mock_map) = mock_registry {
                let lookup_key = match rule.value_name.as_deref() {
                    Some(vn) if !vn.is_empty() => format!("{key_path}::{vn}"),
                    _ => key_path.to_string(),
                };
                mock_map.get(&lookup_key).cloned()
            } else {
                #[cfg(windows)]
                {
                    read_windows_registry_value(key_path, rule.value_name.as_deref())
                }
                #[cfg(not(windows))]
                {
                    None
                }
            };

            let Some(actual) = val_opt else {
                return false;
            };
            let Some(expected) = expected_ver else {
                return true;
            };
            version_satisfies(&actual, expected, op)
        }
        "package" => {
            let target = rule
                .package_name
                .as_deref()
                .or(rule.path.as_deref())
                .unwrap_or("")
                .trim()
                .to_ascii_lowercase();
            if target.is_empty() {
                return false;
            }
            let matched = installed_software.iter().find(|sw| {
                sw.name.to_ascii_lowercase() == target
                    || sw.package_ref.to_ascii_lowercase() == target
            });
            let Some(sw) = matched else {
                return false;
            };
            let Some(expected) = expected_ver else {
                return true;
            };
            version_satisfies(&sw.version, expected, op)
        }
        _ => false,
    }
}

pub fn evaluate_detection_rule(rule: &DetectionRule, installed_software: &[SoftwareEntry]) -> bool {
    evaluate_detection_rule_with(rule, installed_software, None)
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    #[test]
    fn test_windows_software_detection_rules_file_registry_and_package() {
        let dir = tempdir().unwrap();
        let ver_file = dir.path().join("app.version");
        fs::write(&ver_file, "24.08.0\n").unwrap();

        let file_rule = DetectionRule {
            rule_type: "file".to_string(),
            path: Some(ver_file.to_string_lossy().to_string()),
            version: Some("24.08".to_string()),
            operator: Some("gte".to_string()),
            ..Default::default()
        };
        assert!(evaluate_detection_rule(&file_rule, &[]));

        let newer_rule = DetectionRule {
            version: Some("25.00".to_string()),
            ..file_rule
        };
        assert!(!evaluate_detection_rule(&newer_rule, &[]));

        // Registry detection rule with mock hive
        let mut mock_reg = HashMap::new();
        mock_reg.insert(
            r"HKLM\SOFTWARE\7-Zip::DisplayVersion".to_string(),
            "24.08".to_string(),
        );
        let reg_rule = DetectionRule {
            rule_type: "registry".to_string(),
            registry_key: Some(r"HKLM\SOFTWARE\7-Zip".to_string()),
            value_name: Some("DisplayVersion".to_string()),
            version: Some("24.07".to_string()),
            operator: Some("gte".to_string()),
            ..Default::default()
        };
        assert!(evaluate_detection_rule_with(&reg_rule, &[], Some(&mock_reg)));

        // Package detection rule
        let inv = vec![SoftwareEntry {
            name: "Mozilla Firefox".to_string(),
            version: "132.0.1".to_string(),
            publisher: "Mozilla".to_string(),
            source: "winget".to_string(),
            scope: "machine".to_string(),
            package_ref: "Mozilla.Firefox".to_string(),
        }];
        let pkg_rule = DetectionRule {
            rule_type: "package".to_string(),
            package_name: Some("Mozilla.Firefox".to_string()),
            version: Some("132.0".to_string()),
            ..Default::default()
        };
        assert!(evaluate_detection_rule(&pkg_rule, &inv));
    }

    #[test]
    fn test_windows_software_blocked_process_guard() {
        let blocked = vec!["firefox.exe".to_string(), "WINWORD.EXE".to_string()];
        let running = vec!["explorer.exe".to_string(), "firefox".to_string()];
        let found = find_running_blocked_processes_in_list(&blocked, &running);
        assert_eq!(found, vec!["firefox.exe".to_string()]);
    }
}
