use anyhow::{bail, Context, Result};
use std::fs;
use std::path::{Path, PathBuf};
use swm_common::verify_file_sha256;

/// Checks whether `header` begins with a valid native executable or script signature:
/// - Windows PE (`MZ`)
/// - Linux ELF (`\x7fELF`)
/// - macOS Mach-O 64/32-bit (`\xCF\xFA\xED\xFE`, `\xFE\xED\xFA\xCF`, `\xCE\xFA\xED\xFE`, `\xFE\xED\xFA\xCE`)
/// - macOS Universal / Fat binary (`\xCA\xFE\xBA\xBE`, `\xBE\xBA\xFE\xCA`, `\xCA\xFE\xBA\xBF`, `\xBF\xBA\xFE\xCA`)
/// - POSIX / batch test scripts (`#!`, `@echo`)
pub fn has_valid_executable_magic(header: &[u8]) -> bool {
    header.starts_with(b"MZ")
        || header.starts_with(b"\x7fELF")
        || header.starts_with(b"\xCF\xFA\xED\xFE")
        || header.starts_with(b"\xFE\xED\xFA\xCF")
        || header.starts_with(b"\xCE\xFA\xED\xFE")
        || header.starts_with(b"\xFE\xED\xFA\xCE")
        || header.starts_with(b"\xCA\xFE\xBA\xBE")
        || header.starts_with(b"\xBE\xBA\xFE\xCA")
        || header.starts_with(b"\xCA\xFE\xBA\xBF")
        || header.starts_with(b"\xBF\xBA\xFE\xCA")
        || header.starts_with(b"#!")
        || header.starts_with(b"@echo")
}

/// Returns the platform-specific service restart command `(program, args)` after a self-update.
pub fn build_self_update_restart_command(os_family: &str) -> (&'static str, Vec<&'static str>) {
    match os_family.trim().to_ascii_lowercase().as_str() {
        "macos" | "darwin" => (
            "/bin/launchctl",
            vec!["kickstart", "-k", "system/org.nethserver.swm-agent"],
        ),
        "windows" => ("sc.exe", vec!["start", "NS8SoftwareManagerAgent"]),
        _ => ("systemctl", vec!["restart", "swm-agent.service"]),
    }
}

/// Schedules an asynchronous daemon restart ~2 seconds after `post_job_result` completes
/// when running as an installed OS service (`launchd` on macOS, `systemd` on Linux).
pub fn schedule_agent_service_restart() {
    if std::env::var("SWM_DISABLE_SELF_RESTART")
        .ok()
        .as_deref()
        == Some("1")
    {
        return;
    }

    #[cfg(target_os = "macos")]
    {
        if std::path::Path::new(crate::service::LAUNCHD_DAEMON_PLIST_PATH).exists() {
            let _ = std::process::Command::new("/bin/sh")
                .args([
                    "-c",
                    "sleep 2; /bin/launchctl kickstart -k system/org.nethserver.swm-agent >/dev/null 2>&1",
                ])
                .spawn();
        }
    }

    #[cfg(all(unix, not(target_os = "macos")))]
    {
        if std::path::Path::new(crate::service::SYSTEMD_UNIT_PATH).exists() {
            let _ = std::process::Command::new("/bin/sh")
                .args([
                    "-c",
                    "sleep 2; systemctl restart swm-agent.service >/dev/null 2>&1",
                ])
                .spawn();
        }
    }
}

/// Verifies `expected_sha256` and executable binary header of `staged_binary`,
/// then performs an atomic swap of `target_binary` with `.old` backup & automatic rollback on error.
pub fn stage_and_swap_binary(
    staged_binary: &Path,
    expected_sha256: &str,
    target_binary: &Path,
) -> Result<PathBuf> {
    if expected_sha256.trim().is_empty() {
        bail!("Self-update rejected: expected_sha256 must not be empty");
    }

    verify_file_sha256(staged_binary, expected_sha256)
        .map_err(|e| anyhow::anyhow!("Self-update SHA-256 verification failed: {e}"))?;

    let meta = fs::metadata(staged_binary)
        .with_context(|| format!("Missing staged binary {}", staged_binary.display()))?;
    if meta.len() < 4 {
        bail!("Self-update rejected: downloaded binary is truncated (< 4 bytes)");
    }

    // Basic binary header sanity check (PE, ELF, Mach-O / Universal Fat binary, or script in tests)
    let header = fs::read(staged_binary)?;
    if !has_valid_executable_magic(&header) {
        bail!("Self-update rejected: unrecognized executable header magic");
    }

    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        fs::set_permissions(staged_binary, fs::Permissions::from_mode(0o755))?;
    }

    let backup_path = target_binary.with_extension("old");
    if backup_path.exists() {
        let _ = fs::remove_file(&backup_path);
    }

    let had_existing = target_binary.exists();
    if had_existing {
        fs::rename(target_binary, &backup_path).with_context(|| {
            format!(
                "Failed moving existing binary {} to backup {}",
                target_binary.display(),
                backup_path.display()
            )
        })?;
    }

    let swap_res = fs::rename(staged_binary, target_binary)
        .or_else(|_| fs::copy(staged_binary, target_binary).map(|_| ()));

    if let Err(err) = swap_res {
        // Rollback immediately so service executable is never missing!
        if had_existing && backup_path.exists() {
            let _ = fs::rename(&backup_path, target_binary);
        }
        return Err(err).with_context(|| {
            format!(
                "Failed placing new agent binary at {} (rolled back to previous binary)",
                target_binary.display()
            )
        });
    }

    Ok(backup_path)
}

#[cfg(test)]
mod tests {
    use super::*;
    use swm_common::compute_bytes_sha256;
    use tempfile::tempdir;

    #[test]
    fn test_reboot_policy_and_self_update_atomic_swap_and_tamper_rejection() {
        let dir = tempdir().unwrap();
        let current_exe = dir.path().join("swm-agent.exe");
        let staged_new = dir.path().join("swm-agent-new.exe");

        let old_bytes = b"MZ-old-agent-v1.0.0";
        let new_bytes = b"MZ-new-agent-v1.1.0";
        fs::write(&current_exe, old_bytes).unwrap();
        fs::write(&staged_new, new_bytes).unwrap();

        // Wrong SHA-256 leaves current_exe completely untouched
        let bad = stage_and_swap_binary(
            &staged_new,
            "0000000000000000000000000000000000000000000000000000000000000000",
            &current_exe,
        );
        assert!(bad.is_err());
        assert_eq!(fs::read(&current_exe).unwrap(), old_bytes);

        // Valid SHA-256 atomically swaps binary and preserves .old backup
        let good_sha = compute_bytes_sha256(new_bytes);
        let backup = stage_and_swap_binary(&staged_new, &good_sha, &current_exe).unwrap();
        assert_eq!(fs::read(&current_exe).unwrap(), new_bytes);
        assert_eq!(fs::read(&backup).unwrap(), old_bytes);

        // macOS Universal (Fat) and Mach-O 64-bit headers are accepted + launchctl restart command
        assert!(has_valid_executable_magic(b"\xCA\xFE\xBA\xBE\x00\x00\x00\x02"));
        assert!(has_valid_executable_magic(b"\xCF\xFA\xED\xFE\x0C\x00\x00\x01"));
        let (prog, args) = build_self_update_restart_command("macos");
        assert_eq!(prog, "/bin/launchctl");
        assert_eq!(
            args,
            vec!["kickstart", "-k", "system/org.nethserver.swm-agent"]
        );
    }
}
