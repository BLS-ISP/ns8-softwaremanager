pub mod detection;
pub mod linux_installer;
pub mod windows_installer;
pub mod winget;

use anyhow::{bail, Context, Result};
use std::collections::HashMap;
use std::fs::{self, File};
use std::io;
use std::path::{Path, PathBuf};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SoftwareRunOutcome {
    pub succeeded: bool,
    pub exit_code: i32,
    pub reboot_required: bool,
    pub waiting_for_process: bool,
    pub stdout: String,
    pub stderr: String,
}

/// Maps installer exit codes to `(succeeded, reboot_required)` using job-specific `return_codes`
/// plus standard defaults (`0` -> success, `3010`/`1641` -> success + reboot_required).
pub fn map_exit_code(exit_code: i32, return_codes: &HashMap<String, String>) -> (bool, bool) {
    if let Some(mapped) = return_codes.get(&exit_code.to_string()) {
        match mapped.trim().to_ascii_lowercase().as_str() {
            "success" | "succeeded" | "ok" => return (true, false),
            "reboot_required" | "soft_reboot" | "hard_reboot" | "reboot" => return (true, true),
            "failed" | "error" => return (false, false),
            _ => {}
        }
    }

    match exit_code {
        0 => (true, false),
        3010 | 1641 => (true, true),
        _ => (false, false),
    }
}

/// Extracts a `.zip` archive into `dest_dir` while strictly preventing Zip-Slip path traversal (`..`).
pub fn extract_zip_safely(zip_path: &Path, dest_dir: &Path) -> Result<()> {
    if dest_dir.exists() {
        let _ = fs::remove_dir_all(dest_dir);
    }
    fs::create_dir_all(dest_dir)?;

    let file = File::open(zip_path)
        .with_context(|| format!("Failed opening zip archive {}", zip_path.display()))?;
    let mut archive = zip::ZipArchive::new(file).context("Invalid zip archive")?;

    for idx in 0..archive.len() {
        let mut entry = archive.by_index(idx)?;
        let Some(enclosed_rel) = entry.enclosed_name().map(PathBuf::from) else {
            bail!("Security violation: zip entry contains unsafe path traversal");
        };
        let out_path = dest_dir.join(enclosed_rel);
        if entry.is_dir() {
            fs::create_dir_all(&out_path)?;
        } else {
            if let Some(parent) = out_path.parent() {
                fs::create_dir_all(parent)?;
            }
            let mut outfile = File::create(&out_path)?;
            io::copy(&mut entry, &mut outfile)?;
        }
    }
    Ok(())
}
