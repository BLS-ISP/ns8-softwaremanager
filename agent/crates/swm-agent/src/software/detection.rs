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
    #[cfg(windows)]
    {
        let ext = path
            .extension()
            .and_then(|e| e.to_str())
            .unwrap_or("")
            .to_ascii_lowercase();
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
    let expected_ver = rule.effective_version();
    let rule_type_lower = rule.rule_type.trim().to_ascii_lowercase();

    match rule_type_lower.as_str() {
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
                .effective_package()
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
        "app_bundle" => {
            let Some(path_str) = rule.path.as_deref().map(str::trim).filter(|p| !p.is_empty())
            else {
                return false;
            };
            let bundle_path = Path::new(path_str);
            let plist_path = if bundle_path.ends_with("Info.plist") {
                bundle_path.to_path_buf()
            } else {
                bundle_path.join("Contents").join("Info.plist")
            };
            let Some(dict) = crate::inventory::software::read_macos_info_plist_dict(&plist_path)
            else {
                return false;
            };

            if let Some(exp_bundle_id) = rule
                .bundle_id
                .as_deref()
                .map(str::trim)
                .filter(|b| !b.is_empty())
            {
                let actual_bundle_id = dict
                    .get("CFBundleIdentifier")
                    .map(|s| s.trim())
                    .unwrap_or("");
                if !actual_bundle_id.eq_ignore_ascii_case(exp_bundle_id) {
                    return false;
                }
            }

            let Some(expected) = expected_ver else {
                return true;
            };
            if op.eq_ignore_ascii_case("exists") {
                return true;
            }
            let Some(actual_ver) = dict
                .get("CFBundleShortVersionString")
                .map(|s| s.trim())
                .filter(|s| !s.is_empty())
                .or_else(|| {
                    dict.get("CFBundleVersion")
                        .map(|s| s.trim())
                        .filter(|s| !s.is_empty())
                })
            else {
                return false;
            };
            let _ = swm_common::compare_versions(actual_ver, expected);
            version_satisfies(actual_ver, expected, op)
        }
        "pkgutil" => {
            let Some(pkg_id) = rule.effective_package() else {
                return false;
            };
            let mut found_ver: Option<String> = None;

            if let Some(mock_map) = mock_registry {
                if let Some(raw_out) = mock_map.get(&format!("pkgutil::{pkg_id}")) {
                    found_ver = crate::inventory::software::parse_pkgutil_pkg_info(raw_out)
                        .map(|(_, v)| v)
                        .or_else(|| Some(raw_out.trim().to_string()));
                }
            }

            if found_ver.is_none() {
                found_ver = installed_software
                    .iter()
                    .find(|sw| {
                        sw.source.eq_ignore_ascii_case("pkgutil")
                            && sw.package_ref.eq_ignore_ascii_case(pkg_id)
                    })
                    .map(|sw| sw.version.clone());
            }

            #[cfg(target_os = "macos")]
            if found_ver.is_none() {
                if let Ok(out) = std::process::Command::new("/usr/sbin/pkgutil")
                    .args(["--pkg-info", pkg_id])
                    .output()
                {
                    if out.status.success() {
                        found_ver = crate::inventory::software::parse_pkgutil_pkg_info(
                            &String::from_utf8_lossy(&out.stdout),
                        )
                        .map(|(_, v)| v);
                    }
                }
            }

            let Some(actual_ver) = found_ver else {
                return false;
            };
            let Some(expected) = expected_ver else {
                return true;
            };
            version_satisfies(&actual_ver, expected, op)
        }
        "brew" | "brew_cask" => {
            let Some(pkg) = rule.effective_package() else {
                return false;
            };
            let pkg_short = pkg.rsplit('/').next().unwrap_or(pkg);
            let mut found_ver: Option<String> = None;

            if let Some(mock_map) = mock_registry {
                if let Some(v) = mock_map.get(&format!("{rule_type_lower}::{pkg}")) {
                    found_ver = Some(v.trim().to_string());
                }
            }

            if found_ver.is_none() {
                found_ver = installed_software
                    .iter()
                    .find(|sw| {
                        sw.source.eq_ignore_ascii_case(&rule_type_lower)
                            && (sw.package_ref.eq_ignore_ascii_case(pkg)
                                || sw.package_ref.eq_ignore_ascii_case(pkg_short)
                                || sw.name.eq_ignore_ascii_case(pkg)
                                || sw.name.eq_ignore_ascii_case(pkg_short))
                    })
                    .map(|sw| sw.version.clone());
            }

            #[cfg(target_os = "macos")]
            if found_ver.is_none() {
                if let Some(brew_bin) = crate::software::macos_installer::locate_brew_binary() {
                    if let Ok(owner) = crate::software::macos_installer::resolve_brew_owner(&brew_bin)
                    {
                        let kind_flag = if rule_type_lower == "brew_cask" {
                            "--cask"
                        } else {
                            "--formula"
                        };
                        let brew_str = brew_bin.to_string_lossy().to_string();
                        if let Ok(out) = std::process::Command::new("/usr/bin/sudo")
                            .args([
                                "-u",
                                &owner,
                                "-H",
                                "env",
                                "NONINTERACTIVE=1",
                                "HOMEBREW_NO_AUTO_UPDATE=1",
                                &brew_str,
                                "list",
                                kind_flag,
                                "--versions",
                                pkg,
                            ])
                            .current_dir("/tmp")
                            .output()
                        {
                            if out.status.success() {
                                found_ver = crate::inventory::software::parse_brew_list_versions(
                                    &String::from_utf8_lossy(&out.stdout),
                                    &rule_type_lower,
                                )
                                .into_iter()
                                .next()
                                .map(|sw| sw.version);
                            }
                        }
                    }
                }
            }

            let Some(actual_ver) = found_ver else {
                return false;
            };
            let Some(expected) = expected_ver else {
                return true;
            };
            version_satisfies(&actual_ver, expected, op)
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

    #[test]
    fn test_macos_detection_rules_app_bundle_pkgutil_brew_and_brew_cask() {
        let dir = tempdir().unwrap();
        let app_dir = dir.path().join("Firefox.app");
        fs::create_dir_all(app_dir.join("Contents")).unwrap();
        fs::write(
            app_dir.join("Contents").join("Info.plist"),
            r#"<?xml version="1.0" encoding="UTF-8"?>
<plist version="1.0">
<dict>
    <key>CFBundleDisplayName</key>
    <string>Firefox</string>
    <key>CFBundleIdentifier</key>
    <string>org.mozilla.firefox</string>
    <key>CFBundleShortVersionString</key>
    <string>132.0.2</string>
</dict>
</plist>"#,
        )
        .unwrap();

        let app_ok = DetectionRule {
            rule_type: "app_bundle".to_string(),
            path: Some(app_dir.to_string_lossy().to_string()),
            bundle_id: Some("org.mozilla.firefox".to_string()),
            min_version: Some("132.0".to_string()),
            ..Default::default()
        };
        assert!(evaluate_detection_rule(&app_ok, &[]));

        let app_too_old = DetectionRule {
            min_version: Some("133.0".to_string()),
            ..app_ok.clone()
        };
        assert!(!evaluate_detection_rule(&app_too_old, &[]));

        let app_wrong_bundle = DetectionRule {
            bundle_id: Some("com.google.Chrome".to_string()),
            ..app_ok
        };
        assert!(!evaluate_detection_rule(&app_wrong_bundle, &[]));

        // pkgutil rule via mock pkg-info & software inventory
        let mut mock_map = HashMap::new();
        mock_map.insert(
            "pkgutil::us.zoom.pkg.videomeeting".to_string(),
            "package-id: us.zoom.pkg.videomeeting\nversion: 6.2.6.41824\n".to_string(),
        );
        let pkgutil_rule = DetectionRule {
            rule_type: "pkgutil".to_string(),
            package_id: Some("us.zoom.pkg.videomeeting".to_string()),
            min_version: Some("6.2.0".to_string()),
            ..Default::default()
        };
        assert!(evaluate_detection_rule_with(&pkgutil_rule, &[], Some(&mock_map)));

        // brew & brew_cask rules against installed software inventory
        let macos_inv = vec![
            SoftwareEntry {
                name: "jq".to_string(),
                version: "1.7.1".to_string(),
                publisher: "Homebrew".to_string(),
                source: "brew".to_string(),
                scope: "machine".to_string(),
                package_ref: "jq".to_string(),
            },
            SoftwareEntry {
                name: "visual-studio-code".to_string(),
                version: "1.95.3".to_string(),
                publisher: "Homebrew".to_string(),
                source: "brew_cask".to_string(),
                scope: "machine".to_string(),
                package_ref: "visual-studio-code".to_string(),
            },
        ];

        let brew_rule = DetectionRule {
            rule_type: "brew".to_string(),
            package: Some("jq".to_string()),
            min_version: Some("1.7".to_string()),
            ..Default::default()
        };
        assert!(evaluate_detection_rule(&brew_rule, &macos_inv));

        let cask_rule = DetectionRule {
            rule_type: "brew_cask".to_string(),
            package: Some("visual-studio-code".to_string()),
            min_version: Some("1.95.0".to_string()),
            ..Default::default()
        };
        assert!(evaluate_detection_rule(&cask_rule, &macos_inv));
        assert!(!evaluate_detection_rule(
            &DetectionRule {
                rule_type: "brew".to_string(),
                package: Some("visual-studio-code".to_string()),
                ..Default::default()
            },
            &macos_inv
        ));
    }
}
