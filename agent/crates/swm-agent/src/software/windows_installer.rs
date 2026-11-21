use crate::software::{extract_zip_safely, map_exit_code, SoftwareRunOutcome};
use anyhow::{Context, Result};
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::time::Duration;
use tokio::process::Command;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WindowsExecPlan {
    pub program: String,
    pub args: Vec<String>,
    pub working_dir: PathBuf,
}

/// Builds the silent Windows installer invocation plan for `.msi`, `.ps1`, `.bat`/`.cmd`, and `.exe`.
pub fn build_windows_installer_plan(
    installer_path: &Path,
    action: &str,
    custom_args: &str,
) -> WindowsExecPlan {
    let ext = installer_path
        .extension()
        .and_then(|e| e.to_str())
        .unwrap_or("")
        .to_ascii_lowercase();
    let working_dir = installer_path
        .parent()
        .map(Path::to_path_buf)
        .unwrap_or_else(|| PathBuf::from("."));
    let path_str = installer_path.to_string_lossy().to_string();
    let is_uninstall = action.trim().eq_ignore_ascii_case("uninstall");

    match ext.as_str() {
        "msi" => {
            let mut args = Vec::new();
            let trimmed_custom = custom_args.trim();
            // If admin provided full msiexec flags starting with /x or /i, honor them directly
            if trimmed_custom.starts_with("/x")
                || trimmed_custom.starts_with("/X")
                || trimmed_custom.starts_with("/i")
                || trimmed_custom.starts_with("/I")
            {
                for tok in trimmed_custom.split_whitespace() {
                    args.push(tok.to_string());
                }
            } else {
                args.push(if is_uninstall { "/x" } else { "/i" }.to_string());
                args.push(path_str);
                if trimmed_custom.is_empty() {
                    args.push("/qn".to_string());
                    args.push("/norestart".to_string());
                } else {
                    for tok in trimmed_custom.split_whitespace() {
                        args.push(tok.to_string());
                    }
                }
            }
            WindowsExecPlan {
                program: "msiexec.exe".to_string(),
                args,
                working_dir,
            }
        }
        "ps1" => {
            let mut args = vec![
                "-NoProfile".to_string(),
                "-NonInteractive".to_string(),
                "-ExecutionPolicy".to_string(),
                "Bypass".to_string(),
                "-File".to_string(),
                path_str,
            ];
            for tok in custom_args.split_whitespace() {
                if !tok.is_empty() {
                    args.push(tok.to_string());
                }
            }
            WindowsExecPlan {
                program: "powershell.exe".to_string(),
                args,
                working_dir,
            }
        }
        "bat" | "cmd" => {
            let mut args = vec!["/C".to_string(), path_str];
            for tok in custom_args.split_whitespace() {
                if !tok.is_empty() {
                    args.push(tok.to_string());
                }
            }
            WindowsExecPlan {
                program: "cmd.exe".to_string(),
                args,
                working_dir,
            }
        }
        _ => {
            let mut args = Vec::new();
            for tok in custom_args.split_whitespace() {
                if !tok.is_empty() {
                    args.push(tok.to_string());
                }
            }
            WindowsExecPlan {
                program: path_str,
                args,
                working_dir,
            }
        }
    }
}

pub async fn run_windows_custom_package(
    package_file: &Path,
    action: &str,
    custom_command: &str,
    return_codes: &HashMap<String, String>,
    timeout: Duration,
) -> Result<SoftwareRunOutcome> {
    let ext = package_file
        .extension()
        .and_then(|e| e.to_str())
        .unwrap_or("")
        .to_ascii_lowercase();

    let plan = if ext == "zip" {
        let extract_dir = package_file.with_extension("extracted");
        extract_zip_safely(package_file, &extract_dir)?;

        if !custom_command.trim().is_empty() {
            // Check if custom_command names a file directly inside extract_dir
            let candidate = extract_dir.join(custom_command.trim());
            if candidate.is_file() {
                build_windows_installer_plan(&candidate, action, "")
            } else {
                WindowsExecPlan {
                    program: "cmd.exe".to_string(),
                    args: vec!["/C".to_string(), custom_command.trim().to_string()],
                    working_dir: extract_dir,
                }
            }
        } else {
            // Auto-discover install.ps1, setup.bat, install.cmd, or first .msi/.exe
            let discovered = ["install.ps1", "setup.ps1", "install.cmd", "install.bat", "setup.exe"]
                .iter()
                .map(|name| extract_dir.join(name))
                .find(|p| p.is_file())
                .or_else(|| {
                    std::fs::read_dir(&extract_dir)
                        .ok()?
                        .flatten()
                        .map(|e| e.path())
                        .find(|p| {
                            p.extension()
                                .and_then(|e| e.to_str())
                                .map(|s| matches!(s.to_ascii_lowercase().as_str(), "msi" | "ps1" | "exe" | "cmd" | "bat"))
                                .unwrap_or(false)
                        })
                })
                .context("No executable entrypoint found inside extracted .zip package")?;
            build_windows_installer_plan(&discovered, action, "")
        }
    } else {
        build_windows_installer_plan(package_file, action, custom_command)
    };

    let mut cmd = Command::new(&plan.program);
    cmd.args(&plan.args)
        .current_dir(&plan.working_dir)
        .kill_on_drop(true);

    let output = match tokio::time::timeout(timeout, cmd.output()).await {
        Ok(res) => res.with_context(|| format!("Failed executing {}", plan.program))?,
        Err(_) => {
            return Ok(SoftwareRunOutcome {
                succeeded: false,
                exit_code: 1460,
                reboot_required: false,
                waiting_for_process: false,
                stdout: String::new(),
                stderr: format!(
                    "Windows installer timed out after {}s",
                    timeout.as_secs()
                ),
            });
        }
    };

    let code = output.status.code().unwrap_or(1);
    let (succeeded, reboot_required) = map_exit_code(code, return_codes);

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
    use std::io::Write;
    use tempfile::tempdir;

    #[test]
    fn test_windows_software_msi_ps1_and_exit_code_3010_mapping() {
        let msi_plan =
            build_windows_installer_plan(Path::new(r"C:\staging\7z2408-x64.msi"), "install", "");
        assert_eq!(msi_plan.program, "msiexec.exe");
        assert_eq!(
            msi_plan.args,
            vec!["/i", r"C:\staging\7z2408-x64.msi", "/qn", "/norestart"]
        );

        let ps1_plan = build_windows_installer_plan(
            Path::new(r"C:\staging\setup.ps1"),
            "install",
            "-Silent",
        );
        assert_eq!(ps1_plan.program, "powershell.exe");
        assert!(ps1_plan.args.contains(&"Bypass".to_string()));
        assert!(ps1_plan.args.contains(&"-Silent".to_string()));

        let custom_codes = HashMap::new();
        assert_eq!(map_exit_code(0, &custom_codes), (true, false));
        assert_eq!(map_exit_code(3010, &custom_codes), (true, true));
        assert_eq!(map_exit_code(1641, &custom_codes), (true, true));
        assert_eq!(map_exit_code(1603, &custom_codes), (false, false));
    }

    #[cfg(windows)]
    #[tokio::test]
    async fn test_windows_software_ps1_and_zip_execution_with_3010() {
        let dir = tempdir().unwrap();
        let marker = dir.path().join("installed_marker.txt");
        let ps1_path = dir.path().join("install_test.ps1");
        std::fs::write(
            &ps1_path,
            format!(
                "Set-Content -LiteralPath '{}' -Value '1.0.0'\nexit 3010\n",
                marker.display()
            ),
        )
        .unwrap();

        let res = run_windows_custom_package(
            &ps1_path,
            "install",
            "",
            &HashMap::new(),
            Duration::from_secs(15),
        )
        .await
        .unwrap();

        assert!(res.succeeded);
        assert!(res.reboot_required);
        assert_eq!(res.exit_code, 3010);
        assert_eq!(std::fs::read_to_string(&marker).unwrap().trim(), "1.0.0");

        // Also test .zip extraction + execution
        let zip_path = dir.path().join("bundle.zip");
        {
            let file = std::fs::File::create(&zip_path).unwrap();
            let mut zip = zip::ZipWriter::new(file);
            zip.start_file("install.cmd", zip::write::FileOptions::default())
                .unwrap();
            zip.write_all(b"@echo off\r\necho ZIP_OK\r\nexit /b 0\r\n")
                .unwrap();
            zip.finish().unwrap();
        }
        let zip_res = run_windows_custom_package(
            &zip_path,
            "install",
            "",
            &HashMap::new(),
            Duration::from_secs(15),
        )
        .await
        .unwrap();
        assert!(zip_res.succeeded);
        assert!(!zip_res.reboot_required);
        assert!(zip_res.stdout.contains("ZIP_OK"));
    }
}
