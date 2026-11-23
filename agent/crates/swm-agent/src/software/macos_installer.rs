use crate::software::{map_exit_code, SoftwareRunOutcome};
use anyhow::{bail, Context, Result};
use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf};
use std::time::Duration;
use tokio::process::Command;

pub const HOMEBREW_BIN_CANDIDATES: [&str; 2] = ["/opt/homebrew/bin/brew", "/usr/local/bin/brew"];

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MacOsExecPlan {
    pub program: String,
    pub args: Vec<String>,
    pub envs: Vec<(String, String)>,
    pub working_dir: PathBuf,
}

/// Validates that `user` is a safe, non-root macOS user account suitable for `sudo -u <owner> -H`.
/// Homebrew strictly refuses to run as `root`!
pub fn is_valid_non_root_macos_user(user: &str) -> bool {
    let trimmed = user.trim();
    if trimmed.is_empty() {
        return false;
    }
    let lower = trimmed.to_ascii_lowercase();
    if matches!(
        lower.as_str(),
        "root" | "_mbsetupuser" | "loginwindow" | "nobody" | "daemon" | "system"
    ) {
        return false;
    }
    trimmed
        .chars()
        .all(|c| c.is_ascii_alphanumeric() || matches!(c, '.' | '_' | '-'))
}

/// Locates the first existing `brew` executable among `candidates`.
pub fn locate_brew_binary_in(candidates: &[&Path]) -> Option<PathBuf> {
    for cand in candidates {
        if cand.is_file() {
            return Some(cand.to_path_buf());
        }
    }
    None
}

/// Locates Homebrew on Apple Silicon (`/opt/homebrew/bin/brew`) or Intel (`/usr/local/bin/brew`).
pub fn locate_brew_binary() -> Option<PathBuf> {
    if let Ok(override_bin) = std::env::var("SWM_BREW_BIN") {
        let p = PathBuf::from(override_bin.trim());
        if p.is_file() {
            return Some(p);
        }
    }
    let refs: Vec<&Path> = HOMEBREW_BIN_CANDIDATES.iter().map(Path::new).collect();
    locate_brew_binary_in(&refs)
}

/// Resolves the non-root owner of `brew_bin` via `stat -f %Su <brew_bin>`,
/// falling back to the active console user (`stat -f %Su /dev/console`).
/// Never returns `root`.
pub fn resolve_brew_owner(brew_bin: &Path) -> Result<String> {
    if let Ok(override_owner) = std::env::var("SWM_BREW_OWNER") {
        if is_valid_non_root_macos_user(&override_owner) {
            return Ok(override_owner.trim().to_string());
        }
    }

    if let Ok(out) = std::process::Command::new("/usr/bin/stat")
        .args(["-f", "%Su", &brew_bin.to_string_lossy()])
        .output()
    {
        if out.status.success() {
            let owner = String::from_utf8_lossy(&out.stdout).trim().to_string();
            if is_valid_non_root_macos_user(&owner) {
                return Ok(owner);
            }
        }
    }

    if let Ok(out) = std::process::Command::new("/usr/bin/stat")
        .args(["-f", "%Su", "/dev/console"])
        .output()
    {
        if out.status.success() {
            let console_user = String::from_utf8_lossy(&out.stdout).trim().to_string();
            if is_valid_non_root_macos_user(&console_user) {
                return Ok(console_user);
            }
        }
    }

    bail!(
        "Refusing to run Homebrew as root: could not determine a non-root owner for {} or /dev/console",
        brew_bin.display()
    )
}

/// Builds the non-interactive Homebrew (`brew` or `brew_cask`) execution plan under `sudo -u <owner> -H`:
/// `sudo -u <owner> -H env NONINTERACTIVE=1 HOMEBREW_NO_AUTO_UPDATE=1 <brew> <install|upgrade|uninstall> [--cask] <package_ref>`
pub fn build_brew_exec_plan(
    brew_bin: &Path,
    owner: &str,
    source_type: &str,
    action: &str,
    package_ref: &str,
    extra_args: &str,
) -> Result<MacOsExecPlan> {
    if !is_valid_non_root_macos_user(owner) {
        bail!("Security violation: Homebrew owner '{owner}' is invalid or root");
    }

    let pkg = package_ref.trim();
    if pkg.is_empty() || pkg.starts_with('-') {
        bail!("Invalid Homebrew package_ref '{pkg}'");
    }

    let is_cask = match source_type.trim().to_ascii_lowercase().as_str() {
        "brew" => false,
        "brew_cask" => true,
        other => bail!("Unsupported Homebrew source_type '{other}'"),
    };

    let verb = match action.trim().to_ascii_lowercase().as_str() {
        "uninstall" | "remove" => "uninstall",
        "update" | "upgrade" => "upgrade",
        _ => "install",
    };

    let mut args = vec![
        "-u".to_string(),
        owner.trim().to_string(),
        "-H".to_string(),
        "env".to_string(),
        "NONINTERACTIVE=1".to_string(),
        "HOMEBREW_NO_AUTO_UPDATE=1".to_string(),
        brew_bin.to_string_lossy().to_string(),
        verb.to_string(),
    ];
    if is_cask {
        args.push("--cask".to_string());
    }
    args.push(pkg.to_string());

    for tok in extra_args.split_whitespace() {
        if !tok.is_empty() {
            args.push(tok.to_string());
        }
    }

    Ok(MacOsExecPlan {
        program: "/usr/bin/sudo".to_string(),
        args,
        envs: vec![],
        working_dir: PathBuf::from("/tmp"),
    })
}

/// Builds `/usr/sbin/installer -pkg <file> -target /` for macOS `.pkg` installers.
pub fn build_macos_pkg_installer_plan(pkg_file: &Path) -> MacOsExecPlan {
    MacOsExecPlan {
        program: "/usr/sbin/installer".to_string(),
        args: vec![
            "-pkg".to_string(),
            pkg_file.to_string_lossy().to_string(),
            "-target".to_string(),
            "/".to_string(),
        ],
        envs: vec![],
        working_dir: PathBuf::from("/tmp"),
    }
}

/// Builds `/usr/bin/hdiutil attach -nobrowse -readonly -mountpoint <mount_dir> <dmg_file>`.
pub fn build_macos_dmg_attach_plan(dmg_file: &Path, mount_dir: &Path) -> MacOsExecPlan {
    MacOsExecPlan {
        program: "/usr/bin/hdiutil".to_string(),
        args: vec![
            "attach".to_string(),
            "-nobrowse".to_string(),
            "-readonly".to_string(),
            "-mountpoint".to_string(),
            mount_dir.to_string_lossy().to_string(),
            dmg_file.to_string_lossy().to_string(),
        ],
        envs: vec![],
        working_dir: PathBuf::from("/tmp"),
    }
}

/// Builds `/usr/bin/hdiutil detach -force <mount_dir>` for guaranteed `.dmg` mount cleanup.
pub fn build_macos_dmg_detach_plan(mount_dir: &Path) -> MacOsExecPlan {
    MacOsExecPlan {
        program: "/usr/bin/hdiutil".to_string(),
        args: vec![
            "detach".to_string(),
            "-force".to_string(),
            mount_dir.to_string_lossy().to_string(),
        ],
        envs: vec![],
        working_dir: PathBuf::from("/tmp"),
    }
}

/// Builds `/usr/bin/ditto -x -k <zip_file> <dest_dir>` for macOS `.zip` extraction.
pub fn build_macos_ditto_extract_zip_plan(zip_file: &Path, dest_dir: &Path) -> MacOsExecPlan {
    MacOsExecPlan {
        program: "/usr/bin/ditto".to_string(),
        args: vec![
            "-x".to_string(),
            "-k".to_string(),
            zip_file.to_string_lossy().to_string(),
            dest_dir.to_string_lossy().to_string(),
        ],
        envs: vec![],
        working_dir: PathBuf::from("/tmp"),
    }
}

/// Builds `/usr/bin/ditto <src_app> <dest_app>` for copying `.app` bundles while preserving resource forks & signatures.
pub fn build_macos_ditto_copy_app_plan(src_app: &Path, dest_app: &Path) -> MacOsExecPlan {
    MacOsExecPlan {
        program: "/usr/bin/ditto".to_string(),
        args: vec![
            src_app.to_string_lossy().to_string(),
            dest_app.to_string_lossy().to_string(),
        ],
        envs: vec![],
        working_dir: PathBuf::from("/tmp"),
    }
}

/// Builds `/usr/bin/xattr -dr com.apple.quarantine <target_path>`.
pub fn build_macos_strip_quarantine_plan(target_path: &Path) -> MacOsExecPlan {
    MacOsExecPlan {
        program: "/usr/bin/xattr".to_string(),
        args: vec![
            "-dr".to_string(),
            "com.apple.quarantine".to_string(),
            target_path.to_string_lossy().to_string(),
        ],
        envs: vec![],
        working_dir: PathBuf::from("/tmp"),
    }
}

/// Builds `/bin/bash <script_file> <custom_args...>` for `.sh` custom packages.
pub fn build_macos_sh_plan(script_file: &Path, custom_args: &str) -> MacOsExecPlan {
    let working_dir = script_file
        .parent()
        .map(Path::to_path_buf)
        .unwrap_or_else(|| PathBuf::from("/tmp"));
    let mut args = vec![script_file.to_string_lossy().to_string()];
    for tok in custom_args.split_whitespace() {
        if !tok.is_empty() {
            args.push(tok.to_string());
        }
    }
    MacOsExecPlan {
        program: "/bin/bash".to_string(),
        args,
        envs: vec![],
        working_dir,
    }
}

/// Discovers `.pkg` and `.app` payloads inside a mounted `.dmg` or extracted `.zip` directory
/// (up to 2 levels deep), strictly skipping symlinks (such as the standard `Applications -> /Applications`
/// drag-and-drop symlink inside `.dmg` images) and `__MACOSX` metadata folders.
pub fn discover_macos_payloads(root: &Path) -> (Vec<PathBuf>, Vec<PathBuf>) {
    let mut pkgs = Vec::new();
    let mut apps = Vec::new();

    fn scan_dir(dir: &Path, depth: usize, pkgs: &mut Vec<PathBuf>, apps: &mut Vec<PathBuf>) {
        if depth > 2 {
            return;
        }
        let Ok(entries) = fs::read_dir(dir) else {
            return;
        };
        for entry in entries.flatten() {
            let Ok(ft) = entry.file_type() else {
                continue;
            };
            // Never traverse symlinks (e.g. `.dmg` `Applications` symlink pointing to `/Applications`)!
            if ft.is_symlink() {
                continue;
            }
            let path = entry.path();
            let name = entry.file_name().to_string_lossy().to_string();
            if name.starts_with('.') || name == "__MACOSX" {
                continue;
            }
            let ext = path
                .extension()
                .and_then(|e| e.to_str())
                .unwrap_or("")
                .to_ascii_lowercase();
            if (ft.is_file() || ft.is_dir()) && (ext == "pkg" || ext == "mpkg") {
                pkgs.push(path);
            } else if ft.is_dir() && ext == "app" {
                apps.push(path);
            } else if ft.is_dir() {
                scan_dir(&path, depth + 1, pkgs, apps);
            }
        }
    }

    scan_dir(root, 0, &mut pkgs, &mut apps);
    pkgs.sort();
    apps.sort();
    (pkgs, apps)
}

pub async fn execute_macos_plan(
    plan: &MacOsExecPlan,
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
                stderr: format!("macOS command {} timed out after {}s", plan.program, timeout.as_secs()),
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

/// Executes Homebrew (`brew` or `brew_cask`) `install`, `upgrade`, or `uninstall` as the non-root Homebrew owner.
pub async fn run_brew_action(
    source_type: &str,
    action: &str,
    package_ref: &str,
    extra_args: &str,
    return_codes: &HashMap<String, String>,
    timeout: Duration,
) -> Result<SoftwareRunOutcome> {
    let brew_bin = locate_brew_binary().context(
        "Homebrew is not installed on this Mac (expected /opt/homebrew/bin/brew or /usr/local/bin/brew)",
    )?;
    let owner = resolve_brew_owner(&brew_bin)?;
    let plan = build_brew_exec_plan(&brew_bin, &owner, source_type, action, package_ref, extra_args)?;
    execute_macos_plan(&plan, return_codes, timeout).await
}

async fn install_discovered_macos_payloads(
    pkgs: &[PathBuf],
    apps: &[PathBuf],
    applications_dir: &Path,
    return_codes: &HashMap<String, String>,
    timeout: Duration,
) -> Result<SoftwareRunOutcome> {
    let mut combined_stdout = String::new();
    let mut combined_stderr = String::new();
    let mut any_reboot = false;

    if !pkgs.is_empty() {
        for pkg in pkgs {
            let res = execute_macos_plan(
                &build_macos_pkg_installer_plan(pkg),
                return_codes,
                timeout,
            )
            .await?;
            combined_stdout.push_str(&res.stdout);
            combined_stderr.push_str(&res.stderr);
            any_reboot |= res.reboot_required;
            if !res.succeeded {
                return Ok(SoftwareRunOutcome {
                    succeeded: false,
                    exit_code: res.exit_code,
                    reboot_required: any_reboot,
                    waiting_for_process: false,
                    stdout: combined_stdout,
                    stderr: combined_stderr,
                });
            }
        }
        return Ok(SoftwareRunOutcome {
            succeeded: true,
            exit_code: 0,
            reboot_required: any_reboot,
            waiting_for_process: false,
            stdout: combined_stdout,
            stderr: combined_stderr,
        });
    }

    if !apps.is_empty() {
        fs::create_dir_all(applications_dir)?;
        for app in apps {
            let Some(app_name) = app.file_name() else {
                continue;
            };
            let dest_app = applications_dir.join(app_name);
            if dest_app.exists() {
                let _ = fs::remove_dir_all(&dest_app);
            }
            let copy_res = execute_macos_plan(
                &build_macos_ditto_copy_app_plan(app, &dest_app),
                &HashMap::new(),
                timeout,
            )
            .await?;
            combined_stdout.push_str(&copy_res.stdout);
            combined_stderr.push_str(&copy_res.stderr);
            if !copy_res.succeeded {
                return Ok(SoftwareRunOutcome {
                    succeeded: false,
                    exit_code: copy_res.exit_code,
                    reboot_required: false,
                    waiting_for_process: false,
                    stdout: combined_stdout,
                    stderr: combined_stderr,
                });
            }

            // Strip Gatekeeper quarantine attribute (non-fatal if attribute is absent)
            let _ = execute_macos_plan(
                &build_macos_strip_quarantine_plan(&dest_app),
                &HashMap::new(),
                Duration::from_secs(30),
            )
            .await;
            combined_stdout.push_str(&format!("Installed {}\n", dest_app.display()));
        }

        return Ok(SoftwareRunOutcome {
            succeeded: true,
            exit_code: 0,
            reboot_required: false,
            waiting_for_process: false,
            stdout: combined_stdout,
            stderr: combined_stderr,
        });
    }

    bail!("No .pkg or .app bundle found inside macOS archive")
}

/// Installs or uninstalls a custom macOS package (`.pkg`, `.dmg`, `.zip`, `.sh`).
pub async fn run_macos_custom_package(
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
    let is_uninstall = action.trim().eq_ignore_ascii_case("uninstall");
    let applications_dir = PathBuf::from("/Applications");

    // If uninstalling a non-.sh package and admin provided an explicit uninstall shell command, run it
    if is_uninstall && ext != "sh" && !custom_command.trim().is_empty() {
        let plan = MacOsExecPlan {
            program: "/bin/bash".to_string(),
            args: vec!["-c".to_string(), custom_command.trim().to_string()],
            envs: vec![],
            working_dir: PathBuf::from("/tmp"),
        };
        return execute_macos_plan(&plan, return_codes, timeout).await;
    }

    match ext.as_str() {
        "pkg" | "mpkg" => {
            let plan = build_macos_pkg_installer_plan(package_file);
            execute_macos_plan(&plan, return_codes, timeout).await
        }

        "dmg" => {
            let mount_dir = tempfile::Builder::new()
                .prefix("ns8-swm-dmg-")
                .tempdir()
                .context("Failed creating temporary .dmg mount directory")?;
            let mount_path = mount_dir.path().to_path_buf();

            let attach_res = execute_macos_plan(
                &build_macos_dmg_attach_plan(package_file, &mount_path),
                &HashMap::new(),
                Duration::from_secs(120),
            )
            .await?;

            if !attach_res.succeeded {
                let _ = execute_macos_plan(
                    &build_macos_dmg_detach_plan(&mount_path),
                    &HashMap::new(),
                    Duration::from_secs(30),
                )
                .await;
                return Ok(attach_res);
            }

            let inner_res = async {
                let (pkgs, apps) = discover_macos_payloads(&mount_path);
                if is_uninstall && !apps.is_empty() {
                    for app in &apps {
                        if let Some(name) = app.file_name() {
                            let target = applications_dir.join(name);
                            if target.exists() {
                                fs::remove_dir_all(&target)?;
                            }
                        }
                    }
                    return Ok(SoftwareRunOutcome {
                        succeeded: true,
                        exit_code: 0,
                        reboot_required: false,
                        waiting_for_process: false,
                        stdout: "Removed application bundle from /Applications.".to_string(),
                        stderr: String::new(),
                    });
                }
                install_discovered_macos_payloads(
                    &pkgs,
                    &apps,
                    &applications_dir,
                    return_codes,
                    timeout,
                )
                .await
            }
            .await;

            // ALWAYS detach the mounted .dmg image before returning or dropping mount_dir!
            let _ = execute_macos_plan(
                &build_macos_dmg_detach_plan(&mount_path),
                &HashMap::new(),
                Duration::from_secs(30),
            )
            .await;

            inner_res
        }

        "zip" => {
            let extract_dir = tempfile::Builder::new()
                .prefix("ns8-swm-zip-")
                .tempdir()
                .context("Failed creating temporary .zip extraction directory")?;
            let extract_path = extract_dir.path().to_path_buf();

            let extract_res = execute_macos_plan(
                &build_macos_ditto_extract_zip_plan(package_file, &extract_path),
                &HashMap::new(),
                timeout,
            )
            .await?;
            if !extract_res.succeeded {
                return Ok(extract_res);
            }

            let (pkgs, apps) = discover_macos_payloads(&extract_path);
            if is_uninstall && !apps.is_empty() {
                for app in &apps {
                    if let Some(name) = app.file_name() {
                        let target = applications_dir.join(name);
                        if target.exists() {
                            fs::remove_dir_all(&target)?;
                        }
                    }
                }
                return Ok(SoftwareRunOutcome {
                    succeeded: true,
                    exit_code: 0,
                    reboot_required: false,
                    waiting_for_process: false,
                    stdout: "Removed application bundle from /Applications.".to_string(),
                    stderr: String::new(),
                });
            }

            if !apps.is_empty() || !pkgs.is_empty() {
                return install_discovered_macos_payloads(
                    &pkgs,
                    &apps,
                    &applications_dir,
                    return_codes,
                    timeout,
                )
                .await;
            }

            if !custom_command.trim().is_empty() {
                let plan = MacOsExecPlan {
                    program: "/bin/bash".to_string(),
                    args: vec!["-c".to_string(), custom_command.trim().to_string()],
                    envs: vec![],
                    working_dir: extract_path,
                };
                return execute_macos_plan(&plan, return_codes, timeout).await;
            }

            let fallback_script = extract_path.join("install.sh");
            if fallback_script.is_file() {
                return execute_macos_plan(
                    &build_macos_sh_plan(&fallback_script, ""),
                    return_codes,
                    timeout,
                )
                .await;
            }

            bail!("No .app bundle, .pkg installer, or install.sh found inside .zip archive")
        }

        _ => {
            let plan = build_macos_sh_plan(package_file, custom_command);
            execute_macos_plan(&plan, return_codes, timeout).await
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    #[test]
    fn test_macos_installer_brew_and_cask_plans_reject_root() {
        let brew = Path::new("/opt/homebrew/bin/brew");
        assert!(build_brew_exec_plan(brew, "root", "brew", "install", "wget", "").is_err());
        assert!(build_brew_exec_plan(brew, "_mbsetupuser", "brew", "install", "wget", "").is_err());
        assert!(build_brew_exec_plan(brew, "alice", "brew", "install", "--evil-flag", "").is_err());

        let formula_plan =
            build_brew_exec_plan(brew, "alice", "brew", "install", "jq", "").unwrap();
        assert_eq!(formula_plan.program, "/usr/bin/sudo");
        assert_eq!(
            formula_plan.args,
            vec![
                "-u",
                "alice",
                "-H",
                "env",
                "NONINTERACTIVE=1",
                "HOMEBREW_NO_AUTO_UPDATE=1",
                "/opt/homebrew/bin/brew",
                "install",
                "jq",
            ]
        );

        let cask_upgrade = build_brew_exec_plan(
            Path::new("/usr/local/bin/brew"),
            "bob",
            "brew_cask",
            "update",
            "firefox",
            "",
        )
        .unwrap();
        assert_eq!(
            cask_upgrade.args,
            vec![
                "-u",
                "bob",
                "-H",
                "env",
                "NONINTERACTIVE=1",
                "HOMEBREW_NO_AUTO_UPDATE=1",
                "/usr/local/bin/brew",
                "upgrade",
                "--cask",
                "firefox",
            ]
        );

        let cask_uninstall =
            build_brew_exec_plan(brew, "alice", "brew_cask", "uninstall", "vlc", "").unwrap();
        assert_eq!(
            cask_uninstall.args,
            vec![
                "-u",
                "alice",
                "-H",
                "env",
                "NONINTERACTIVE=1",
                "HOMEBREW_NO_AUTO_UPDATE=1",
                "/opt/homebrew/bin/brew",
                "uninstall",
                "--cask",
                "vlc",
            ]
        );
    }

    #[test]
    fn test_macos_installer_pkg_dmg_zip_quarantine_and_payload_discovery() {
        let pkg_plan = build_macos_pkg_installer_plan(Path::new("/tmp/Zoom.pkg"));
        assert_eq!(pkg_plan.program, "/usr/sbin/installer");
        assert_eq!(pkg_plan.args, vec!["-pkg", "/tmp/Zoom.pkg", "-target", "/"]);

        let attach = build_macos_dmg_attach_plan(
            Path::new("/tmp/Firefox.dmg"),
            Path::new("/tmp/mnt-1"),
        );
        assert_eq!(attach.program, "/usr/bin/hdiutil");
        assert_eq!(
            attach.args,
            vec![
                "attach",
                "-nobrowse",
                "-readonly",
                "-mountpoint",
                "/tmp/mnt-1",
                "/tmp/Firefox.dmg"
            ]
        );

        let detach = build_macos_dmg_detach_plan(Path::new("/tmp/mnt-1"));
        assert_eq!(detach.program, "/usr/bin/hdiutil");
        assert_eq!(detach.args, vec!["detach", "-force", "/tmp/mnt-1"]);

        let ditto_zip = build_macos_ditto_extract_zip_plan(
            Path::new("/tmp/App.zip"),
            Path::new("/tmp/ext-1"),
        );
        assert_eq!(ditto_zip.program, "/usr/bin/ditto");
        assert_eq!(ditto_zip.args, vec!["-x", "-k", "/tmp/App.zip", "/tmp/ext-1"]);

        let copy_app = build_macos_ditto_copy_app_plan(
            Path::new("/tmp/mnt-1/Firefox.app"),
            Path::new("/Applications/Firefox.app"),
        );
        assert_eq!(copy_app.program, "/usr/bin/ditto");
        assert_eq!(
            copy_app.args,
            vec!["/tmp/mnt-1/Firefox.app", "/Applications/Firefox.app"]
        );

        let xattr = build_macos_strip_quarantine_plan(Path::new("/Applications/Firefox.app"));
        assert_eq!(xattr.program, "/usr/bin/xattr");
        assert_eq!(
            xattr.args,
            vec!["-dr", "com.apple.quarantine", "/Applications/Firefox.app"]
        );

        // Verify payload discovery finds .app & .pkg while ignoring __MACOSX and dot folders
        let tmp = tempdir().unwrap();
        fs::create_dir_all(tmp.path().join("Firefox.app").join("Contents")).unwrap();
        fs::create_dir_all(tmp.path().join("__MACOSX").join("Ghost.app")).unwrap();
        fs::create_dir_all(tmp.path().join(".background")).unwrap();
        fs::write(tmp.path().join("Helper.pkg"), b"pkg").unwrap();

        let (pkgs, apps) = discover_macos_payloads(tmp.path());
        assert_eq!(pkgs.len(), 1);
        assert!(pkgs[0].ends_with("Helper.pkg"));
        assert_eq!(apps.len(), 1);
        assert!(apps[0].ends_with("Firefox.app"));
    }
}
