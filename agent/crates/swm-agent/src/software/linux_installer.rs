use crate::software::{extract_zip_safely, map_exit_code, SoftwareRunOutcome};
use anyhow::{Context, Result};
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::time::Duration;
use tokio::process::Command;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LinuxExecPlan {
    pub program: String,
    pub args: Vec<String>,
    pub envs: Vec<(String, String)>,
    pub working_dir: PathBuf,
}

/// Builds non-interactive Linux native package manager commands (`apt`, `dnf`, `flatpak`).
pub fn build_linux_native_plan(
    source_type: &str,
    action: &str,
    package_ref: &str,
    extra_args: &str,
) -> Result<LinuxExecPlan> {
    let act = action.trim().to_ascii_lowercase();
    let pkg = package_ref.trim().to_string();

    match source_type.trim().to_ascii_lowercase().as_str() {
        "apt" => {
            let mut args = match act.as_str() {
                "uninstall" | "remove" => vec!["remove".to_string(), "-y".to_string(), pkg],
                "update" | "upgrade" => vec![
                    "install".to_string(),
                    "--only-upgrade".to_string(),
                    "-y".to_string(),
                    "-o".to_string(),
                    "Dpkg::Options::=--force-confdef".to_string(),
                    "-o".to_string(),
                    "Dpkg::Options::=--force-confold".to_string(),
                    pkg,
                ],
                _ => vec![
                    "install".to_string(),
                    "-y".to_string(),
                    "-o".to_string(),
                    "Dpkg::Options::=--force-confdef".to_string(),
                    "-o".to_string(),
                    "Dpkg::Options::=--force-confold".to_string(),
                    pkg,
                ],
            };
            for tok in extra_args.split_whitespace() {
                args.push(tok.to_string());
            }
            Ok(LinuxExecPlan {
                program: "apt-get".to_string(),
                args,
                envs: vec![("DEBIAN_FRONTEND".to_string(), "noninteractive".to_string())],
                working_dir: PathBuf::from("/"),
            })
        }
        "dnf" => {
            let verb = match act.as_str() {
                "uninstall" | "remove" => "remove",
                "update" | "upgrade" => "upgrade",
                _ => "install",
            };
            let mut args = vec![verb.to_string(), "-y".to_string(), pkg];
            for tok in extra_args.split_whitespace() {
                args.push(tok.to_string());
            }
            Ok(LinuxExecPlan {
                program: "dnf".to_string(),
                args,
                envs: vec![],
                working_dir: PathBuf::from("/"),
            })
        }
        "flatpak" => {
            let mut args = match act.as_str() {
                "uninstall" | "remove" => {
                    vec![
                        "uninstall".to_string(),
                        "--system".to_string(),
                        "-y".to_string(),
                        pkg,
                    ]
                }
                "update" | "upgrade" => {
                    vec![
                        "update".to_string(),
                        "--system".to_string(),
                        "-y".to_string(),
                        pkg,
                    ]
                }
                _ => vec![
                    "install".to_string(),
                    "--system".to_string(),
                    "-y".to_string(),
                    pkg,
                ],
            };
            for tok in extra_args.split_whitespace() {
                args.push(tok.to_string());
            }
            Ok(LinuxExecPlan {
                program: "flatpak".to_string(),
                args,
                envs: vec![],
                working_dir: PathBuf::from("/"),
            })
        }
        other => anyhow::bail!("Unsupported Linux native source_type '{other}'"),
    }
}

/// Builds non-interactive Linux custom file installer plans (`.deb`, `.rpm`, `.sh`).
pub fn build_linux_custom_file_plan(
    package_file: &Path,
    custom_args: &str,
) -> Result<LinuxExecPlan> {
    let ext = package_file
        .extension()
        .and_then(|e| e.to_str())
        .unwrap_or("")
        .to_ascii_lowercase();
    let working_dir = package_file
        .parent()
        .map(Path::to_path_buf)
        .unwrap_or_else(|| PathBuf::from("/tmp"));

    // apt-get and dnf require `./` or `/` prefix to treat argument as local package file
    let raw_str = package_file.to_string_lossy();
    let local_file_arg =
        if raw_str.starts_with('/') || raw_str.starts_with("./") || package_file.is_absolute() {
            raw_str.to_string()
        } else {
            format!("./{raw_str}")
        };

    match ext.as_str() {
        "deb" => {
            let mut args = vec![
                "install".to_string(),
                "-y".to_string(),
                "-o".to_string(),
                "Dpkg::Options::=--force-confdef".to_string(),
                "-o".to_string(),
                "Dpkg::Options::=--force-confold".to_string(),
                local_file_arg,
            ];
            for tok in custom_args.split_whitespace() {
                args.push(tok.to_string());
            }
            Ok(LinuxExecPlan {
                program: "apt-get".to_string(),
                args,
                envs: vec![("DEBIAN_FRONTEND".to_string(), "noninteractive".to_string())],
                working_dir,
            })
        }
        "rpm" => {
            let mut args = vec!["install".to_string(), "-y".to_string(), local_file_arg];
            for tok in custom_args.split_whitespace() {
                args.push(tok.to_string());
            }
            Ok(LinuxExecPlan {
                program: "dnf".to_string(),
                args,
                envs: vec![],
                working_dir,
            })
        }
        _ => {
            let mut args = vec![package_file.to_string_lossy().to_string()];
            for tok in custom_args.split_whitespace() {
                args.push(tok.to_string());
            }
            Ok(LinuxExecPlan {
                program: "bash".to_string(),
                args,
                envs: vec![],
                working_dir,
            })
        }
    }
}

pub async fn execute_linux_plan(
    plan: &LinuxExecPlan,
    return_codes: &HashMap<String, String>,
    timeout: Duration,
) -> Result<SoftwareRunOutcome> {
    let mut cmd = Command::new(&plan.program);
    cmd.args(&plan.args)
        .current_dir(&plan.working_dir)
        .kill_on_drop(true);
    for (k, v) in &plan.envs {
        cmd.env(k, v);
    }

    let output = match tokio::time::timeout(timeout, cmd.output()).await {
        Ok(res) => res.with_context(|| format!("Failed executing {}", plan.program))?,
        Err(_) => {
            return Ok(SoftwareRunOutcome {
                succeeded: false,
                exit_code: 124,
                reboot_required: false,
                waiting_for_process: false,
                stdout: String::new(),
                stderr: format!("Linux installer timed out after {}s", timeout.as_secs()),
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

pub async fn run_linux_custom_package(
    package_file: &Path,
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
            LinuxExecPlan {
                program: "sh".to_string(),
                args: vec!["-c".to_string(), custom_command.trim().to_string()],
                envs: vec![("DEBIAN_FRONTEND".to_string(), "noninteractive".to_string())],
                working_dir: extract_dir,
            }
        } else {
            let script = extract_dir.join("install.sh");
            build_linux_custom_file_plan(&script, "")?
        }
    } else {
        build_linux_custom_file_plan(package_file, custom_command)?
    };

    execute_linux_plan(&plan, return_codes, timeout).await
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_linux_software_apt_dnf_flatpak_and_custom_deb_rpm_plans() {
        let apt_plan = build_linux_native_plan("apt", "install", "htop", "").unwrap();
        assert_eq!(apt_plan.program, "apt-get");
        assert!(apt_plan
            .envs
            .contains(&("DEBIAN_FRONTEND".to_string(), "noninteractive".to_string())));
        assert!(apt_plan
            .args
            .contains(&"Dpkg::Options::=--force-confold".to_string()));

        let fp_plan =
            build_linux_native_plan("flatpak", "install", "org.gimp.GIMP", "").unwrap();
        assert_eq!(
            fp_plan.args,
            vec!["install", "--system", "-y", "org.gimp.GIMP"]
        );

        let deb_plan =
            build_linux_custom_file_plan(Path::new("/var/tmp/agent-pkg.deb"), "").unwrap();
        assert_eq!(deb_plan.program, "apt-get");
        assert!(deb_plan.args.contains(&"/var/tmp/agent-pkg.deb".to_string()));

        let rpm_plan = build_linux_custom_file_plan(Path::new("agent-pkg.rpm"), "").unwrap();
        assert_eq!(rpm_plan.program, "dnf");
        assert_eq!(rpm_plan.args, vec!["install", "-y", "./agent-pkg.rpm"]);
    }
}
