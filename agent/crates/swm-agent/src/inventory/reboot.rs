use std::path::Path;

pub const WIN_CBS_REBOOT_PENDING: &str =
    r"SOFTWARE\Microsoft\Windows\CurrentVersion\Component Based Servicing\RebootPending";
pub const WIN_WU_REBOOT_REQUIRED: &str =
    r"SOFTWARE\Microsoft\Windows\CurrentVersion\WindowsUpdate\Auto Update\RebootRequired";
pub const WIN_SESSION_MANAGER_KEY: &str = r"SYSTEM\CurrentControlSet\Control\Session Manager";
pub const WIN_PENDING_FILE_RENAME_VALUE: &str = "PendingFileRenameOperations";
pub const LINUX_REBOOT_REQUIRED_PATH: &str = "/var/run/reboot-required";

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

    #[cfg(not(windows))]
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
}
