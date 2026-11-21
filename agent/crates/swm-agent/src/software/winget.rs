use crate::software::SoftwareRunOutcome;
use anyhow::{Context, Result};
use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf};
use std::time::Duration;
use swm_common::compare_versions;
use tokio::process::Command;

pub const DEFAULT_WINDOWS_APPS_DIR: &str = r"C:\Program Files\WindowsApps";

/// Extracts the package version from a `Microsoft.DesktopAppInstaller_<version>_<arch>__8wekyb3d8bbwe` folder name.
pub fn extract_desktop_app_installer_version(dir_name: &str) -> Option<String> {
    let rest = dir_name.strip_prefix("Microsoft.DesktopAppInstaller_")?;
    if !rest.ends_with("__8wekyb3d8bbwe") {
        return None;
    }
    let ver = rest.split('_').next()?;
    if ver.is_empty() {
        None
    } else {
        Some(ver.to_string())
    }
}

/// Resolves `winget.exe` safely when running under `NT AUTHORITY\SYSTEM` (Session 0),
/// where `%LOCALAPPDATA%\Microsoft\WindowsApps` is not mapped to the MSIX alias.
/// Selects the highest-versioned `Microsoft.DesktopAppInstaller_*` directory containing `winget.exe`.
pub fn resolve_system_winget_in_dir(windows_apps_dir: &Path, target_arch: &str) -> Option<PathBuf> {
    let entries = fs::read_dir(windows_apps_dir).ok()?;
    let arch_token = if target_arch.eq_ignore_ascii_case("arm64")
        || target_arch.eq_ignore_ascii_case("aarch64")
    {
        "_arm64__"
    } else {
        "_x64__"
    };

    let mut candidates: Vec<(String, PathBuf)> = Vec::new();
    for entry in entries.flatten() {
        let file_name = entry.file_name();
        let name_str = file_name.to_string_lossy();
        if !name_str.starts_with("Microsoft.DesktopAppInstaller_")
            || !name_str.contains(arch_token)
            || !name_str.ends_with("__8wekyb3d8bbwe")
        {
            continue;
        }
        let Some(ver) = extract_desktop_app_installer_version(&name_str) else {
            continue;
        };
        let exe_path = entry.path().join("winget.exe");
        if exe_path.is_file() {
            candidates.push((ver, exe_path));
        }
    }

    candidates.sort_by(|(va, _), (vb, _)| compare_versions(va, vb));
    candidates.pop().map(|(_, path)| path)
}

pub fn resolve_winget_binary() -> Result<PathBuf> {
    if let Ok(override_bin) = std::env::var("SWM_WINGET_BIN") {
        let p = PathBuf::from(override_bin);
        if p.exists() {
            return Ok(p);
        }
    }

    if let Some(sys_path) =
        resolve_system_winget_in_dir(Path::new(DEFAULT_WINDOWS_APPS_DIR), std::env::consts::ARCH)
    {
        return Ok(sys_path);
    }

    if let Ok(local_app_data) = std::env::var("LOCALAPPDATA") {
        let alias = PathBuf::from(local_app_data)
            .join("Microsoft")
            .join("WindowsApps")
            .join("winget.exe");
        if alias.exists() {
            return Ok(alias);
        }
    }

    anyhow::bail!(
        "Could not locate winget.exe under {} or LOCALAPPDATA",
        DEFAULT_WINDOWS_APPS_DIR
    )
}

/// Builds SYSTEM-safe non-interactive `winget.exe` CLI arguments enforcing `--scope machine --silent`.
pub fn build_winget_args(
    action: &str,
    package_ref: &str,
    version: &str,
    extra_args: &str,
) -> Vec<String> {
    let verb = match action.trim().to_ascii_lowercase().as_str() {
        "uninstall" | "remove" => "uninstall",
        "update" | "upgrade" => "upgrade",
        _ => "install",
    };

    let mut args = vec![
        verb.to_string(),
        "--id".to_string(),
        package_ref.trim().to_string(),
        "--exact".to_string(),
        "--scope".to_string(),
        "machine".to_string(),
        "--silent".to_string(),
        "--disable-interactivity".to_string(),
        "--accept-source-agreements".to_string(),
    ];

    if verb != "uninstall" {
        args.push("--accept-package-agreements".to_string());
    }

    if verb == "install" && !version.trim().is_empty() && !version.eq_ignore_ascii_case("latest") {
        args.push("--version".to_string());
        args.push(version.trim().to_string());
    }

    for token in extra_args.split_whitespace() {
        if !token.is_empty() {
            args.push(token.to_string());
        }
    }

    args
}

pub async fn run_winget_action(
    winget_bin: &Path,
    action: &str,
    package_ref: &str,
    version: &str,
    extra_args: &str,
    return_codes: &HashMap<String, String>,
    timeout: Duration,
) -> Result<SoftwareRunOutcome> {
    let args = build_winget_args(action, package_ref, version, extra_args);

    let mut cmd = Command::new(winget_bin);
    cmd.args(&args).kill_on_drop(true);

    let output = match tokio::time::timeout(timeout, cmd.output()).await {
        Ok(res) => res.with_context(|| format!("Failed spawning {}", winget_bin.display()))?,
        Err(_) => {
            return Ok(SoftwareRunOutcome {
                succeeded: false,
                exit_code: 1460, // ERROR_TIMEOUT
                reboot_required: false,
                waiting_for_process: false,
                stdout: String::new(),
                stderr: format!(
                    "winget execution timed out after {}s (aborted Session 0 hang)",
                    timeout.as_secs()
                ),
            });
        }
    };

    let code = output.status.code().unwrap_or(1);
    let (succeeded, reboot_required) = crate::software::map_exit_code(code, return_codes);

    Ok(SoftwareRunOutcome {
        succeeded,
        exit_code: code,
        reboot_required,
        waiting_for_process: false,
        stdout: String::from_utf8_lossy(&output.stdout).to_string(),
        stderr: String::from_utf8_lossy(&output.stderr).to_string(),
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    #[test]
    fn test_windows_software_winget_system_resolver_picks_latest_version() {
        let dir = tempdir().unwrap();
        let older = dir
            .path()
            .join("Microsoft.DesktopAppInstaller_1.22.10861.0_x64__8wekyb3d8bbwe");
        let newer = dir
            .path()
            .join("Microsoft.DesktopAppInstaller_1.24.25200.0_x64__8wekyb3d8bbwe");
        let arm = dir
            .path()
            .join("Microsoft.DesktopAppInstaller_1.29.99999.0_arm64__8wekyb3d8bbwe");

        fs::create_dir_all(&older).unwrap();
        fs::create_dir_all(&newer).unwrap();
        fs::create_dir_all(&arm).unwrap();

        fs::write(older.join("winget.exe"), b"MZ-old").unwrap();
        fs::write(newer.join("winget.exe"), b"MZ-new").unwrap();
        fs::write(arm.join("winget.exe"), b"MZ-arm").unwrap();

        let resolved = resolve_system_winget_in_dir(dir.path(), "x86_64").unwrap();
        assert_eq!(resolved, newer.join("winget.exe"));

        let resolved_arm = resolve_system_winget_in_dir(dir.path(), "aarch64").unwrap();
        assert_eq!(resolved_arm, arm.join("winget.exe"));
    }

    #[test]
    fn test_windows_software_winget_enforces_machine_scope_and_agreements() {
        let args = build_winget_args("install", "7zip.7zip", "24.08", "");
        assert!(args.windows(2).any(|w| w == ["--scope", "machine"]));
        assert!(args.contains(&"--silent".to_string()));
        assert!(args.contains(&"--accept-package-agreements".to_string()));
        assert!(args.contains(&"--accept-source-agreements".to_string()));
        assert!(args.windows(2).any(|w| w == ["--version", "24.08"]));
    }

    #[tokio::test]
    async fn test_windows_software_winget_timeout_aborts_hanging_process() {
        // Spawn a quick cross-platform sleep command via powershell/sleep to verify timeout enforcement
        let bin = if cfg!(windows) {
            PathBuf::from("powershell.exe")
        } else {
            PathBuf::from("sleep")
        };

        let mut cmd = if cfg!(windows) {
            let mut c = Command::new(&bin);
            c.args(["-NoProfile", "-Command", "Start-Sleep -Seconds 10"])
                .kill_on_drop(true);
            c
        } else {
            let mut c = Command::new(&bin);
            c.arg("10").kill_on_drop(true);
            c
        };

        let timed = tokio::time::timeout(Duration::from_millis(120), cmd.output()).await;
        assert!(timed.is_err());
    }
}
