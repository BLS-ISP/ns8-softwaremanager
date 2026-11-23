use std::path::Path;

pub const WIN_CBS_REBOOT_PENDING: &str =
    r"SOFTWARE\Microsoft\Windows\CurrentVersion\Component Based Servicing\RebootPending";
pub const WIN_WU_REBOOT_REQUIRED: &str =
    r"SOFTWARE\Microsoft\Windows\CurrentVersion\WindowsUpdate\Auto Update\RebootRequired";
pub const WIN_SESSION_MANAGER_KEY: &str = r"SYSTEM\CurrentControlSet\Control\Session Manager";
pub const WIN_PENDING_FILE_RENAME_VALUE: &str = "PendingFileRenameOperations";
pub const LINUX_REBOOT_REQUIRED_PATH: &str = "/var/run/reboot-required";
pub const MACOS_UPDATES_INDEX_PLIST: &str = "/Library/Updates/index.plist";

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct WindowsRebootIndicators {
    pub cbs_reboot_pending: bool,
    pub wu_reboot_required: bool,
    pub pending_file_rename_ops: Vec<String>,
}

impl WindowsRebootIndicators {
    pub fn is_reboot_required(&self) -> bool {
        self.cbs_reboot_pending
            || self.wu_reboot_required
            || self
                .pending_file_rename_ops
                .iter()
                .any(|entry| !entry.trim().is_empty())
    }
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct LinuxRebootIndicators {
    pub reboot_required_file_exists: bool,
    pub needs_restarting_exit_code: Option<i32>,
}

impl LinuxRebootIndicators {
    /// On RHEL/Rocky/Alma/Fedora, `needs-restarting -r` returns exit code `1` when reboot is required,
    /// and `0` when no reboot is needed.
    pub fn is_reboot_required(&self) -> bool {
        self.reboot_required_file_exists || self.needs_restarting_exit_code == Some(1)
    }
}

#[cfg(windows)]
pub fn inspect_windows_reboot_indicators() -> WindowsRebootIndicators {
    use winreg::enums::{HKEY_LOCAL_MACHINE, KEY_READ};
    use winreg::RegKey;

    let hklm = RegKey::predef(HKEY_LOCAL_MACHINE);
    let cbs_reboot_pending = hklm
        .open_subkey_with_flags(WIN_CBS_REBOOT_PENDING, KEY_READ)
        .is_ok();
    let wu_reboot_required = hklm
        .open_subkey_with_flags(WIN_WU_REBOOT_REQUIRED, KEY_READ)
        .is_ok();

    let mut pending_file_rename_ops = Vec::new();
    if let Ok(sm) = hklm.open_subkey_with_flags(WIN_SESSION_MANAGER_KEY, KEY_READ) {
        if let Ok(multi) = sm.get_value::<Vec<String>, _>(WIN_PENDING_FILE_RENAME_VALUE) {
            pending_file_rename_ops = multi;
        } else if let Ok(single) = sm.get_value::<String, _>(WIN_PENDING_FILE_RENAME_VALUE) {
            if !single.trim().is_empty() {
                pending_file_rename_ops.push(single);
            }
        }
    }

    WindowsRebootIndicators {
        cbs_reboot_pending,
        wu_reboot_required,
        pending_file_rename_ops,
    }
}

pub fn inspect_linux_reboot_indicators_with(
    reboot_file_path: &Path,
    run_needs_restarting: bool,
) -> LinuxRebootIndicators {
    let reboot_required_file_exists = reboot_file_path.exists();
    let needs_restarting_exit_code = if run_needs_restarting {
        std::process::Command::new("needs-restarting")
            .arg("-r")
            .output()
            .ok()
            .and_then(|o| o.status.code())
    } else {
        None
    };

    LinuxRebootIndicators {
        reboot_required_file_exists,
        needs_restarting_exit_code,
    }
}

/// Parses macOS `/Library/Updates/index.plist` XML content and returns `true` when
/// `InstallAtLogout` contains one or more staged update identifiers requiring a restart/logout.
pub fn parse_macos_updates_index_plist_requires_reboot(plist_xml: &str) -> bool {
    let Some(key_pos) = plist_xml.find("<key>InstallAtLogout</key>") else {
        return false;
    };
    let rest = &plist_xml[key_pos + "<key>InstallAtLogout</key>".len()..];
    let trimmed = rest.trim_start();
    if trimmed.starts_with("<array/>") || trimmed.starts_with("<array />") {
        return false;
    }
    let Some(arr_start) = trimmed.find("<array>") else {
        return false;
    };
    let after_arr = &trimmed[arr_start + "<array>".len()..];
    let Some(arr_end) = after_arr.find("</array>") else {
        return false;
    };
    let inner = &after_arr[..arr_end];
    let mut scan = inner;
    while let Some(s_start) = scan.find("<string>") {
        let after_s = &scan[s_start + "<string>".len()..];
        let Some(s_end) = after_s.find("</string>") else {
            break;
        };
        if !after_s[..s_end].trim().is_empty() {
            return true;
        }
        scan = &after_s[s_end + "</string>".len()..];
    }
    false
}

pub fn inspect_macos_reboot_indicators_with(index_plist_path: &Path) -> bool {
    let Ok(bytes) = std::fs::read(index_plist_path) else {
        return false;
    };

    #[cfg(target_os = "macos")]
    if bytes.starts_with(b"bplist") {
        if let Ok(out) = std::process::Command::new("/usr/bin/plutil")
            .args([
                "-convert",
                "xml1",
                "-o",
                "-",
                &index_plist_path.to_string_lossy(),
            ])
            .output()
        {
            if out.status.success() {
                let xml = String::from_utf8_lossy(&out.stdout);
                return parse_macos_updates_index_plist_requires_reboot(&xml);
            }
        }
        return false;
    }

    let xml = String::from_utf8_lossy(&bytes);
    parse_macos_updates_index_plist_requires_reboot(&xml)
}

/// Cross-platform OS-level pending reboot detector.
/// Supports override via `SWM_FORCE_REBOOT_REQUIRED=1|0` for deterministic E2E tests.
pub fn detect_os_pending_reboot() -> bool {
    if let Ok(forced) = std::env::var("SWM_FORCE_REBOOT_REQUIRED") {
        return matches!(forced.trim(), "1" | "true" | "TRUE" | "yes");
    }

    #[cfg(windows)]
    {
        return inspect_windows_reboot_indicators().is_reboot_required();
    }

    #[cfg(target_os = "macos")]
    {
        return inspect_macos_reboot_indicators_with(Path::new(MACOS_UPDATES_INDEX_PLIST));
    }

    #[cfg(all(unix, not(target_os = "macos")))]
    {
        return inspect_linux_reboot_indicators_with(
            Path::new(LINUX_REBOOT_REQUIRED_PATH),
            true,
        )
        .is_reboot_required();
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    #[test]
    fn test_inventory_windows_reboot_indicators_evaluation() {
        let clean = WindowsRebootIndicators::default();
        assert!(!clean.is_reboot_required());

        let cbs = WindowsRebootIndicators {
            cbs_reboot_pending: true,
            ..Default::default()
        };
        assert!(cbs.is_reboot_required());

        let wu = WindowsRebootIndicators {
            wu_reboot_required: true,
            ..Default::default()
        };
        assert!(wu.is_reboot_required());

        let rename = WindowsRebootIndicators {
            pending_file_rename_ops: vec![r"\??\C:\Windows\Temp\old.dll".to_string()],
            ..Default::default()
        };
        assert!(rename.is_reboot_required());
    }

    #[test]
    fn test_inventory_linux_reboot_indicators_evaluation() {
        let dir = tempdir().unwrap();
        let marker = dir.path().join("reboot-required");
        assert!(
            !inspect_linux_reboot_indicators_with(&marker, false).is_reboot_required()
        );

        std::fs::write(&marker, "*** System restart required ***\n").unwrap();
        assert!(inspect_linux_reboot_indicators_with(&marker, false).is_reboot_required());

        let rhel_reboot = LinuxRebootIndicators {
            reboot_required_file_exists: false,
            needs_restarting_exit_code: Some(1),
        };
        assert!(rhel_reboot.is_reboot_required());

        let rhel_clean = LinuxRebootIndicators {
            reboot_required_file_exists: false,
            needs_restarting_exit_code: Some(0),
        };
        assert!(!rhel_clean.is_reboot_required());
    }

    #[test]
    fn test_inventory_macos_updates_index_plist_reboot_detection() {
        let dir = tempdir().unwrap();
        let index_plist = dir.path().join("index.plist");
        assert!(!inspect_macos_reboot_indicators_with(&index_plist));

        let empty_logout = r#"<?xml version="1.0" encoding="UTF-8"?>
<plist version="1.0">
<dict>
    <key>InstallAtLogout</key>
    <array/>
    <key>ProductPaths</key>
    <dict/>
</dict>
</plist>"#;
        std::fs::write(&index_plist, empty_logout).unwrap();
        assert!(!inspect_macos_reboot_indicators_with(&index_plist));

        let staged_logout = r#"<?xml version="1.0" encoding="UTF-8"?>
<plist version="1.0">
<dict>
    <key>InstallAtLogout</key>
    <array>
        <string>072-01928</string>
    </array>
</dict>
</plist>"#;
        std::fs::write(&index_plist, staged_logout).unwrap();
        assert!(inspect_macos_reboot_indicators_with(&index_plist));
    }
}
