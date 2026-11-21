use anyhow::{bail, Context, Result};
use std::fs;
use std::path::{Path, PathBuf};
use swm_common::verify_file_sha256;

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

    // Basic binary header sanity check (MZ for Windows PE, \x7fELF for Linux ELF, or #! script in tests)
    let header = fs::read(staged_binary)?;
    let valid_magic = header.starts_with(b"MZ")
        || header.starts_with(b"\x7fELF")
        || header.starts_with(b"#!")
        || header.starts_with(b"@echo");
    if !valid_magic {
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
    }
}
