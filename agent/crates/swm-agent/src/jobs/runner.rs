use crate::config::{AgentConfig, AgentRuntimeState};
use crate::inventory::build_inventory_report;
use crate::patching::linux_pkg::{install_linux_os_updates, scan_linux_os_updates};
use crate::patching::macos_update::{install_macos_os_updates, scan_macos_os_updates};
use crate::patching::reboot_manager::{execute_system_reboot, RebootManager};
use crate::patching::windows_wua::{PowerShellWuaProvider, WuaProvider};
use crate::self_update::{schedule_agent_service_restart, stage_and_swap_binary};
use crate::software::detection::{check_blocked_processes, evaluate_detection_rule};
use crate::software::linux_installer::{
    build_linux_native_plan, execute_linux_plan, run_linux_custom_package,
};
use crate::software::macos_installer::{run_brew_action, run_macos_custom_package};
use crate::software::windows_installer::run_windows_custom_package;
use crate::software::winget::{resolve_winget_binary, run_winget_action};
use crate::transport::api_client::AgentApiClient;
use crate::transport::downloader::ResumableDownloader;
use chrono::Utc;
use serde::Deserialize;
use std::path::{Path, PathBuf};
use std::time::Duration;
use swm_common::{
    verify_signed_job, AdminDesktopMessage, JobActivityEntry, JobResult, RebootPolicy, SignedJob,
    WsAgentFrame,
};
use tokio::sync::mpsc;

pub fn sanitize_filename(raw: &str) -> String {
    let base = raw
        .rsplit(['/', '\\'])
        .next()
        .unwrap_or("package.bin")
        .trim();
    let cleaned: String = base
        .chars()
        .filter(|c| c.is_ascii_alphanumeric() || matches!(c, '.' | '-' | '_'))
        .collect();
    if cleaned.is_empty() || cleaned == "." || cleaned == ".." {
        "package.bin".to_string()
    } else {
        cleaned
    }
}

pub struct JobExecutionContext<'a> {
    pub data_dir: &'a Path,
    pub config: &'a AgentConfig,
    pub api: &'a AgentApiClient,
    pub state: &'a mut AgentRuntimeState,
    pub reboot_mgr: &'a mut RebootManager,
    pub ws_log_tx: Option<mpsc::Sender<WsAgentFrame>>,
    pub target_agent_exe: Option<PathBuf>,
}

async fn emit_log_chunks(
    ws_tx: &Option<mpsc::Sender<WsAgentFrame>>,
    job_id: &str,
    stdout: &str,
    stderr: &str,
) {
    let Some(tx) = ws_tx else {
        return;
    };
    if !stdout.is_empty() {
        for chunk in stdout.as_bytes().chunks(4096) {
            let _ = tx
                .send(WsAgentFrame::LogChunk {
                    job_id: job_id.to_string(),
                    stream: "stdout".to_string(),
                    chunk: String::from_utf8_lossy(chunk).to_string(),
                })
                .await;
        }
    }
    if !stderr.is_empty() {
        for chunk in stderr.as_bytes().chunks(4096) {
            let _ = tx
                .send(WsAgentFrame::LogChunk {
                    job_id: job_id.to_string(),
                    stream: "stderr".to_string(),
                    chunk: String::from_utf8_lossy(chunk).to_string(),
                })
                .await;
        }
    }
}

/// Verifies the Ed25519 signature + expiry of `job` and executes it sequentially.
/// Any tampered or expired job is rejected deterministically before touching the OS.
pub async fn verify_and_execute_job(
    ctx: &mut JobExecutionContext<'_>,
    job: &SignedJob,
) -> JobResult {
    let now_unix = Utc::now().timestamp();

    // 1. Mandatory Ed25519 Signature & Expiry Gate
    if let Err(crypto_err) = verify_signed_job(
        job,
        &ctx.config.server_public_key_b64,
        Some(&ctx.config.device_id),
        now_unix,
    ) {
        let msg = format!("SECURITY REJECTION: {crypto_err}");
        tracing::error!(job_id = %job.job_id, "{msg}");
        let rejected_res = JobResult {
            status: "failed".to_string(),
            exit_code: -1,
            reboot_required: false,
            stdout: String::new(),
            stderr: msg.clone(),
        };
        ctx.state.record_activity(JobActivityEntry {
            job_id: job.job_id.clone(),
            job_type: job.job_type.clone(),
            action: job.action.clone(),
            target_name: job.target_id.clone(),
            version: job.version.clone(),
            status: "failed".to_string(),
            exit_code: Some(-1),
            reboot_required: false,
            timestamp: AgentRuntimeState::now_iso8601(),
            message: msg,
        });
        let _ = ctx.state.save(ctx.data_dir);
        return rejected_res;
    }

    // 2. Dispatch verified job by job_type
    let result = match job.job_type.trim().to_ascii_lowercase().as_str() {
        "scan_updates" => execute_scan_updates_job(ctx).await,
        "os_patch" => execute_os_patch_job(ctx, job).await,
        "software" => execute_software_job(ctx, job).await,
        "self_update" => execute_self_update_job(ctx, job).await,
        "remote_control" => execute_remote_control_job(ctx, job),
        other => JobResult {
            status: "failed".to_string(),
            exit_code: 2,
            reboot_required: false,
            stdout: String::new(),
            stderr: format!("Unsupported job_type '{other}'"),
        },
    };

    emit_log_chunks(&ctx.ws_log_tx, &job.job_id, &result.stdout, &result.stderr).await;

    if result.reboot_required {
        let _ = ctx
            .reboot_mgr
            .mark_reboot_required(Some(job.job_id.clone()), Utc::now().timestamp());
        ctx.state.reboot_state = ctx.reboot_mgr.state().clone();
    }

    let target_label = job
        .package_meta
        .as_ref()
        .map(|m| m.name.clone())
        .filter(|n| !n.is_empty())
        .unwrap_or_else(|| job.target_id.clone());

    ctx.state.record_activity(JobActivityEntry {
        job_id: job.job_id.clone(),
        job_type: job.job_type.clone(),
        action: job.action.clone(),
        target_name: target_label,
        version: job.version.clone(),
        status: result.status.clone(),
        exit_code: Some(result.exit_code),
        reboot_required: result.reboot_required,
        timestamp: AgentRuntimeState::now_iso8601(),
        message: if result.status == "succeeded" {
            "Auftrag erfolgreich ausgeführt".to_string()
        } else if !result.stderr.is_empty() {
            result.stderr.lines().next().unwrap_or("").to_string()
        } else {
            result.stdout.lines().next().unwrap_or("").to_string()
        },
    });
    let _ = ctx.state.save(ctx.data_dir);

    result
}

async fn execute_scan_updates_job(ctx: &mut JobExecutionContext<'_>) -> JobResult {
    let scan_res = if cfg!(windows) {
        PowerShellWuaProvider
            .scan(&ctx.state.excluded_updates, Duration::from_secs(60))
            .await
    } else if cfg!(target_os = "macos") {
        scan_macos_os_updates(&ctx.state.excluded_updates).await
    } else {
        scan_linux_os_updates(&ctx.state.excluded_updates).await
    };

    match scan_res {
        Ok(pending) => {
            let count = pending.len();
            ctx.state.pending_os_updates_cache = pending.clone();
            let inv = build_inventory_report(
                ctx.state.last_software_hash.as_deref(),
                false,
                pending,
                ctx.reboot_mgr.state().reboot_required,
            );
            let _ = ctx.api.post_inventory(&inv.report).await;
            ctx.state.last_software_hash = Some(inv.current_software_hash);
            ctx.state.installed_software_cache = inv.all_software;

            JobResult {
                status: "succeeded".to_string(),
                exit_code: 0,
                reboot_required: inv.report.reboot_required,
                stdout: format!("OS update scan completed: {count} pending update(s) found."),
                stderr: String::new(),
            }
        }
        Err(err) => JobResult {
            status: "failed".to_string(),
            exit_code: 1,
            reboot_required: false,
            stdout: String::new(),
            stderr: format!("OS update scan failed: {err}"),
        },
    }
}

async fn execute_os_patch_job(ctx: &mut JobExecutionContext<'_>, job: &SignedJob) -> JobResult {
    let approved_ids: Vec<String> = if !job.update_ids.is_empty() {
        job.update_ids.clone()
    } else if !job.target_id.trim().is_empty() && !job.target_id.eq_ignore_ascii_case("all") {
        job.target_id
            .split(',')
            .map(|s| s.trim().to_string())
            .filter(|s| !s.is_empty())
            .collect()
    } else {
        Vec::new()
    };

    if cfg!(target_os = "macos") {
        let target_ids = if job.security_only {
            match scan_macos_os_updates(&ctx.state.excluded_updates).await {
                Ok(scanned) => {
                    let approved_lower: std::collections::HashSet<String> = approved_ids
                        .iter()
                        .map(|s| s.to_ascii_lowercase())
                        .collect();
                    let sec_ids: Vec<String> = scanned
                        .into_iter()
                        .filter(|u| u.category == "security")
                        .filter(|u| {
                            approved_lower.is_empty()
                                || approved_lower.contains(&u.update_id.to_ascii_lowercase())
                        })
                        .map(|u| u.update_id)
                        .collect();
                    if sec_ids.is_empty() {
                        return JobResult {
                            status: "succeeded".to_string(),
                            exit_code: 0,
                            reboot_required: crate::inventory::reboot::detect_os_pending_reboot(),
                            stdout: "No matching macOS security updates to install.".to_string(),
                            stderr: String::new(),
                        };
                    }
                    sec_ids
                }
                Err(err) => {
                    return JobResult {
                        status: "failed".to_string(),
                        exit_code: 1,
                        reboot_required: false,
                        stdout: String::new(),
                        stderr: format!("macOS security update scan failed: {err}"),
                    };
                }
            }
        } else {
            approved_ids
        };

        return match install_macos_os_updates(&target_ids, &ctx.state.excluded_updates).await {
            Ok(res) => res,
            Err(err) => JobResult {
                status: "failed".to_string(),
                exit_code: 1,
                reboot_required: false,
                stdout: String::new(),
                stderr: format!("macOS OS patching error: {err}"),
            },
        };
    }

    let outcome_res = if cfg!(windows) {
        PowerShellWuaProvider
            .install(
                &approved_ids,
                &ctx.state.excluded_updates,
                job.security_only,
            )
            .await
    } else {
        install_linux_os_updates(
            &approved_ids,
            &ctx.state.excluded_updates,
            job.security_only,
        )
        .await
    };

    match outcome_res {
        Ok(out) => JobResult {
            status: if out.succeeded { "succeeded" } else { "failed" }.to_string(),
            exit_code: out.exit_code,
            reboot_required: out.reboot_required,
            stdout: out.stdout,
            stderr: out.stderr,
        },
        Err(err) => JobResult {
            status: "failed".to_string(),
            exit_code: 1,
            reboot_required: false,
            stdout: String::new(),
            stderr: format!("OS patching error: {err}"),
        },
    }
}

async fn execute_software_job(ctx: &mut JobExecutionContext<'_>, job: &SignedJob) -> JobResult {
    // 1. Check blocked running processes before running install/update/uninstall
    if !job.blocked_processes.is_empty() {
        let running = check_blocked_processes(&job.blocked_processes);
        if !running.is_empty() {
            return JobResult {
                status: "waiting_for_process".to_string(),
                exit_code: 1618,
                reboot_required: false,
                stdout: String::new(),
                stderr: format!(
                    "Waiting for running application(s) to close before continuing: {}",
                    running.join(", ")
                ),
            };
        }
    }

    let is_uninstall = job.action.trim().eq_ignore_ascii_case("uninstall");

    // 2. Pre-install Detection Rule evaluation (skip redundant install if target version already present)
    if !is_uninstall {
        if let Some(rule) = &job.detection_rule {
            if !rule.is_empty()
                && evaluate_detection_rule(rule, &ctx.state.installed_software_cache)
            {
                return JobResult {
                    status: "succeeded".to_string(),
                    exit_code: 0,
                    reboot_required: false,
                    stdout: "Software already satisfies detection rule; skipping installation."
                        .to_string(),
                    stderr: String::new(),
                };
            }
        }
    }

    let source_type = job
        .package_meta
        .as_ref()
        .map(|m| m.source_type.trim().to_ascii_lowercase())
        .filter(|s| !s.is_empty())
        .unwrap_or_else(|| {
            if job.download_url.is_some()
                || job.sha256.as_deref().filter(|s| !s.is_empty()).is_some()
            {
                "custom".to_string()
            } else if cfg!(windows) {
                "winget".to_string()
            } else if cfg!(target_os = "macos") {
                "brew".to_string()
            } else {
                "apt".to_string()
            }
        });

    let package_ref = job
        .package_meta
        .as_ref()
        .map(|m| m.package_ref.trim().to_string())
        .filter(|s| !s.is_empty())
        .unwrap_or_else(|| job.target_id.clone());

    let cmd_args = job
        .command
        .as_deref()
        .filter(|s| !s.trim().is_empty())
        .or_else(|| {
            job.package_meta.as_ref().map(|m| {
                if is_uninstall {
                    m.uninstall_args.as_str()
                } else {
                    m.install_args.as_str()
                }
            })
        })
        .unwrap_or("");

    let timeout = Duration::from_secs(
        std::env::var("SWM_INSTALL_TIMEOUT_SECS")
            .ok()
            .and_then(|v| v.parse().ok())
            .unwrap_or(1800),
    );

    let run_res = if source_type == "custom" {
        let dl_rel = job
            .download_url
            .clone()
            .unwrap_or_else(|| format!("/api/v1/agent/packages/{}/download", job.target_id));
        let full_url = ctx.api.resolve_url(&dl_rel);
        let expected_sha = job.sha256.as_deref().unwrap_or("");

        let raw_filename = job
            .package_meta
            .as_ref()
            .and_then(|m| {
                if !m.filename.trim().is_empty() {
                    Some(m.filename.as_str())
                } else if !m.package_ref.trim().is_empty() {
                    Some(m.package_ref.as_str())
                } else {
                    None
                }
            })
            .unwrap_or("installer.bin");
        let safe_name = sanitize_filename(raw_filename);
        let job_stage_dir = AgentRuntimeState::staging_dir(ctx.data_dir).join(&job.job_id);
        let dest_path = job_stage_dir.join(safe_name);

        let downloader = ResumableDownloader::new(ctx.api.http_client().clone());
        match downloader
            .download_verified(&full_url, ctx.api.device_token(), &dest_path, expected_sha)
            .await
        {
            Ok(verified_file) => {
                if cfg!(windows) {
                    run_windows_custom_package(
                        &verified_file,
                        &job.action,
                        cmd_args,
                        &job.return_codes,
                        timeout,
                    )
                    .await
                } else if cfg!(target_os = "macos") {
                    run_macos_custom_package(
                        &verified_file,
                        &job.action,
                        cmd_args,
                        &job.return_codes,
                        timeout,
                    )
                    .await
                } else {
                    run_linux_custom_package(&verified_file, cmd_args, &job.return_codes, timeout)
                        .await
                }
            }
            Err(dl_err) => {
                return JobResult {
                    status: "failed".to_string(),
                    exit_code: -2,
                    reboot_required: false,
                    stdout: String::new(),
                    stderr: format!("Download / SHA-256 verification failed: {dl_err}"),
                };
            }
        }
    } else if source_type == "winget" {
        match resolve_winget_binary() {
            Ok(bin) => {
                run_winget_action(
                    &bin,
                    &job.action,
                    &package_ref,
                    &job.version,
                    cmd_args,
                    &job.return_codes,
                    timeout,
                )
                .await
            }
            Err(e) => Err(e),
        }
    } else if source_type == "brew" || source_type == "brew_cask" {
        run_brew_action(
            &source_type,
            &job.action,
            &package_ref,
            cmd_args,
            &job.return_codes,
            timeout,
        )
        .await
    } else {
        match build_linux_native_plan(&source_type, &job.action, &package_ref, cmd_args) {
            Ok(plan) => execute_linux_plan(&plan, &job.return_codes, timeout).await,
            Err(e) => Err(e),
        }
    };

    match run_res {
        Ok(outcome) => JobResult {
            status: if outcome.waiting_for_process {
                "waiting_for_process"
            } else if outcome.succeeded {
                "succeeded"
            } else {
                "failed"
            }
            .to_string(),
            exit_code: outcome.exit_code,
            reboot_required: outcome.reboot_required,
            stdout: outcome.stdout,
            stderr: outcome.stderr,
        },
        Err(err) => JobResult {
            status: "failed".to_string(),
            exit_code: 1,
            reboot_required: false,
            stdout: String::new(),
            stderr: format!("Software execution error: {err}"),
        },
    }
}

async fn execute_self_update_job(ctx: &mut JobExecutionContext<'_>, job: &SignedJob) -> JobResult {
    let Some(dl_rel) = job.download_url.as_deref().filter(|u| !u.trim().is_empty()) else {
        return JobResult {
            status: "failed".to_string(),
            exit_code: 1,
            reboot_required: false,
            stdout: String::new(),
            stderr: "Self-update job missing download_url".to_string(),
        };
    };
    let expected_sha = job.sha256.as_deref().unwrap_or("");
    let full_url = ctx.api.resolve_url(dl_rel);
    let stage_path = AgentRuntimeState::staging_dir(ctx.data_dir).join(format!(
        "swm-agent-update-{}{}",
        job.version,
        if cfg!(windows) { ".exe" } else { "" }
    ));

    let downloader = ResumableDownloader::new(ctx.api.http_client().clone());
    if let Err(err) = downloader
        .download_verified(&full_url, ctx.api.device_token(), &stage_path, expected_sha)
        .await
    {
        return JobResult {
            status: "failed".to_string(),
            exit_code: -2,
            reboot_required: false,
            stdout: String::new(),
            stderr: format!("Self-update download/SHA-256 failed: {err}"),
        };
    }

    let target_exe = match ctx.target_agent_exe.clone().or_else(|| std::env::current_exe().ok()) {
        Some(p) => p,
        None => {
            return JobResult {
                status: "failed".to_string(),
                exit_code: 1,
                reboot_required: false,
                stdout: String::new(),
                stderr: "Could not resolve current swm-agent binary path".to_string(),
            };
        }
    };

    match stage_and_swap_binary(&stage_path, expected_sha, &target_exe) {
        Ok(backup) => {
            if ctx.target_agent_exe.is_none() {
                schedule_agent_service_restart();
            }
            JobResult {
                status: "succeeded".to_string(),
                exit_code: 0,
                reboot_required: false,
                stdout: format!(
                    "Agent binary updated to version {} (previous backed up at {})",
                    job.version,
                    backup.display()
                ),
                stderr: String::new(),
            }
        }
        Err(err) => JobResult {
            status: "failed".to_string(),
            exit_code: 1,
            reboot_required: false,
            stdout: String::new(),
            stderr: format!("Self-update swap failed: {err}"),
        },
    }
}

#[derive(Debug, Deserialize)]
struct AdminNotifyPayload {
    pub title: String,
    pub message: String,
    #[serde(default)]
    pub urgent: bool,
}

#[derive(Debug, Deserialize)]
struct RemoteRebootPayload {
    #[serde(default)]
    pub mode: Option<String>,
    #[serde(default)]
    pub grace_minutes: Option<u32>,
}

fn execute_remote_control_job(ctx: &mut JobExecutionContext<'_>, job: &SignedJob) -> JobResult {
    match job.action.trim().to_ascii_lowercase().as_str() {
        "force_sync" => JobResult {
            status: "succeeded".to_string(),
            exit_code: 0,
            reboot_required: false,
            stdout: "Immediate inventory and policy sync triggered.".to_string(),
            stderr: String::new(),
        },

        "admin_notify" => {
            let raw = job.command.as_deref().unwrap_or("");
            let payload: AdminNotifyPayload = match serde_json::from_str(raw) {
                Ok(p) => p,
                Err(err) => {
                    return JobResult {
                        status: "failed".to_string(),
                        exit_code: 1,
                        reboot_required: false,
                        stdout: String::new(),
                        stderr: format!("Invalid admin_notify JSON payload: {err}"),
                    };
                }
            };

            let entry = AdminDesktopMessage {
                id: job.job_id.clone(),
                title: payload.title.clone(),
                message: payload.message,
                urgent: payload.urgent,
                created_at: AgentRuntimeState::now_iso8601(),
            };

            ctx.state.admin_messages.retain(|m| m.id != entry.id);
            ctx.state.admin_messages.insert(0, entry);
            if ctx.state.admin_messages.len() > 10 {
                ctx.state.admin_messages.truncate(10);
            }
            let _ = ctx.state.save(ctx.data_dir);

            JobResult {
                status: "succeeded".to_string(),
                exit_code: 0,
                reboot_required: false,
                stdout: format!("Admin notification '{}' stored for desktop client.", payload.title),
                stderr: String::new(),
            }
        }

        "remote_reboot" => {
            let raw = job.command.as_deref().filter(|s| !s.trim().is_empty()).unwrap_or("{}");
            let payload: RemoteRebootPayload = match serde_json::from_str(raw) {
                Ok(p) => p,
                Err(err) => {
                    return JobResult {
                        status: "failed".to_string(),
                        exit_code: 1,
                        reboot_required: false,
                        stdout: String::new(),
                        stderr: format!("Invalid remote_reboot JSON payload: {err}"),
                    };
                }
            };

            let mode = payload
                .mode
                .as_deref()
                .unwrap_or(&job.target_id)
                .trim()
                .to_ascii_lowercase();
            let now_ts = Utc::now().timestamp();

            match mode.as_str() {
                "cancel" => {
                    if let Err(err) = ctx.reboot_mgr.clear_reboot_state() {
                        return JobResult {
                            status: "failed".to_string(),
                            exit_code: 1,
                            reboot_required: false,
                            stdout: String::new(),
                            stderr: format!("Failed canceling pending reboot: {err}"),
                        };
                    }
                    ctx.state.reboot_state = ctx.reboot_mgr.state().clone();
                    let _ = ctx.state.save(ctx.data_dir);
                    JobResult {
                        status: "succeeded".to_string(),
                        exit_code: 0,
                        reboot_required: false,
                        stdout: "Pending system reboot canceled by administrator.".to_string(),
                        stderr: String::new(),
                    }
                }

                "grace_period" => {
                    let grace = payload.grace_minutes.unwrap_or(60).max(1);
                    let _ = ctx.reboot_mgr.clear_reboot_state();
                    if let Err(err) = ctx.reboot_mgr.sync_policy_and_os_state(
                        RebootPolicy::GracePeriod,
                        grace,
                        true,
                        now_ts,
                    ) {
                        return JobResult {
                            status: "failed".to_string(),
                            exit_code: 1,
                            reboot_required: false,
                            stdout: String::new(),
                            stderr: format!("Failed setting grace_period reboot state: {err}"),
                        };
                    }
                    if let Err(err) = ctx
                        .reboot_mgr
                        .mark_reboot_required(Some(job.job_id.clone()), now_ts)
                    {
                        return JobResult {
                            status: "failed".to_string(),
                            exit_code: 1,
                            reboot_required: false,
                            stdout: String::new(),
                            stderr: format!("Failed marking reboot required: {err}"),
                        };
                    }
                    ctx.state.reboot_state = ctx.reboot_mgr.state().clone();
                    let _ = ctx.state.save(ctx.data_dir);
                    JobResult {
                        status: "succeeded".to_string(),
                        exit_code: 0,
                        reboot_required: true,
                        stdout: format!(
                            "Scheduled remote reboot with {grace}-minute grace period."
                        ),
                        stderr: String::new(),
                    }
                }

                "immediate" => {
                    if let Err(err) = ctx
                        .reboot_mgr
                        .mark_reboot_required(Some(job.job_id.clone()), now_ts)
                    {
                        return JobResult {
                            status: "failed".to_string(),
                            exit_code: 1,
                            reboot_required: false,
                            stdout: String::new(),
                            stderr: format!("Failed marking immediate reboot state: {err}"),
                        };
                    }
                    ctx.state.reboot_state = ctx.reboot_mgr.state().clone();
                    let _ = ctx.state.save(ctx.data_dir);
                    if let Err(err) = execute_system_reboot(ctx.data_dir, "remote_admin_immediate")
                    {
                        return JobResult {
                            status: "failed".to_string(),
                            exit_code: 1,
                            reboot_required: true,
                            stdout: String::new(),
                            stderr: format!("Failed triggering immediate OS reboot: {err}"),
                        };
                    }
                    JobResult {
                        status: "succeeded".to_string(),
                        exit_code: 0,
                        reboot_required: true,
                        stdout: "Immediate system reboot initiated by administrator.".to_string(),
                        stderr: String::new(),
                    }
                }

                other => JobResult {
                    status: "failed".to_string(),
                    exit_code: 1,
                    reboot_required: false,
                    stdout: String::new(),
                    stderr: format!("Unsupported remote_reboot mode '{other}'"),
                },
            }
        }

        other => JobResult {
            status: "failed".to_string(),
            exit_code: 2,
            reboot_required: false,
            stdout: String::new(),
            stderr: format!("Unsupported remote_control action '{other}'"),
        },
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashMap;
    use swm_common::{DetectionRule, PackageMeta, TestSigningKeypair};
    use tempfile::tempdir;

    #[tokio::test]
    async fn test_job_runner_rejects_tampered_and_expired_signatures() {
        let dir = tempdir().unwrap();
        let kp = TestSigningKeypair::generate();
        let cfg = AgentConfig {
            server_url: "http://127.0.0.1:9".to_string(),
            device_id: "dev-1".to_string(),
            device_token: "tok-1".to_string(),
            status: "approved".to_string(),
            server_public_key_b64: kp.public_key_b64(),
            enrolled_at: "2026-11-20T00:00:00Z".to_string(),
        };
        let api = AgentApiClient::new(&cfg.server_url, Some(cfg.device_token.clone())).unwrap();
        let mut state = AgentRuntimeState::default();
        let mut reboot_mgr = RebootManager::load(dir.path());

        let mut tampered_job = SignedJob {
            job_id: "job-tamper-1".to_string(),
            device_id: "dev-1".to_string(),
            job_type: "software".to_string(),
            action: "install".to_string(),
            target_id: "pkg-1".to_string(),
            version: "1.0".to_string(),
            sha256: None,
            command: Some("/S".to_string()),
            expires_at_unix: Utc::now().timestamp() + 3600,
            signature_b64: String::new(),
            download_url: None,
            update_ids: vec![],
            security_only: false,
            package_meta: None,
            detection_rule: None,
            blocked_processes: vec![],
            return_codes: HashMap::new(),
        };
        kp.sign_job(&mut tampered_job);
        // Tamper with command AFTER signing
        tampered_job.command = Some("cmd.exe /c whoami".to_string());

        let mut ctx = JobExecutionContext {
            data_dir: dir.path(),
            config: &cfg,
            api: &api,
            state: &mut state,
            reboot_mgr: &mut reboot_mgr,
            ws_log_tx: None,
            target_agent_exe: None,
        };

        let res = verify_and_execute_job(&mut ctx, &tampered_job).await;
        assert_eq!(res.status, "failed");
        assert_eq!(res.exit_code, -1);
        assert!(res.stderr.contains("SECURITY REJECTION"));
    }

    #[tokio::test]
    async fn test_job_runner_skips_when_detection_rule_already_satisfied() {
        let dir = tempdir().unwrap();
        let marker = dir.path().join("already_there.txt");
        std::fs::write(&marker, "2.5.0\n").unwrap();

        let kp = TestSigningKeypair::generate();
        let cfg = AgentConfig {
            server_url: "http://127.0.0.1:9".to_string(),
            device_id: "dev-1".to_string(),
            device_token: "tok-1".to_string(),
            status: "approved".to_string(),
            server_public_key_b64: kp.public_key_b64(),
            enrolled_at: "2026-11-20T00:00:00Z".to_string(),
        };
        let api = AgentApiClient::new(&cfg.server_url, Some(cfg.device_token.clone())).unwrap();
        let mut state = AgentRuntimeState::default();
        let mut reboot_mgr = RebootManager::load(dir.path());

        let mut job = SignedJob {
            job_id: "job-detect-1".to_string(),
            device_id: "dev-1".to_string(),
            job_type: "software".to_string(),
            action: "install".to_string(),
            target_id: "pkg-detect".to_string(),
            version: "2.5.0".to_string(),
            sha256: None,
            command: None,
            expires_at_unix: Utc::now().timestamp() + 3600,
            signature_b64: String::new(),
            download_url: None,
            update_ids: vec![],
            security_only: false,
            package_meta: Some(PackageMeta {
                name: "TestApp".to_string(),
                source_type: "winget".to_string(),
                package_ref: "Test.App".to_string(),
                ..Default::default()
            }),
            detection_rule: Some(DetectionRule {
                rule_type: "file".to_string(),
                path: Some(marker.to_string_lossy().to_string()),
                version: Some("2.5.0".to_string()),
                ..Default::default()
            }),
            blocked_processes: vec![],
            return_codes: HashMap::new(),
        };
        kp.sign_job(&mut job);

        let mut ctx = JobExecutionContext {
            data_dir: dir.path(),
            config: &cfg,
            api: &api,
            state: &mut state,
            reboot_mgr: &mut reboot_mgr,
            ws_log_tx: None,
            target_agent_exe: None,
        };

        let res = verify_and_execute_job(&mut ctx, &job).await;
        assert_eq!(res.status, "succeeded");
        assert_eq!(res.exit_code, 0);
        assert!(res.stdout.contains("already satisfies detection rule"));
    }

    #[tokio::test]
    async fn test_remote_control_signed_jobs_notify_sync_and_reboot_modes() {
        let dir = tempdir().unwrap();
        let kp = TestSigningKeypair::generate();
        let cfg = AgentConfig {
            server_url: "http://127.0.0.1:9".to_string(),
            device_id: "dev-rc-1".to_string(),
            device_token: "tok-rc-1".to_string(),
            status: "approved".to_string(),
            server_public_key_b64: kp.public_key_b64(),
            enrolled_at: "2026-11-23T00:00:00Z".to_string(),
        };
        let api = AgentApiClient::new(&cfg.server_url, Some(cfg.device_token.clone())).unwrap();
        let mut state = AgentRuntimeState::default();
        let mut reboot_mgr = RebootManager::load(dir.path());

        let make_rc_job = |id: &str, action: &str, target: &str, cmd: &str| {
            let mut j = SignedJob {
                job_id: id.to_string(),
                device_id: "dev-rc-1".to_string(),
                job_type: "remote_control".to_string(),
                action: action.to_string(),
                target_id: target.to_string(),
                version: String::new(),
                sha256: None,
                command: Some(cmd.to_string()),
                expires_at_unix: Utc::now().timestamp() + 3600,
                signature_b64: String::new(),
                download_url: None,
                update_ids: vec![],
                security_only: false,
                package_meta: None,
                detection_rule: None,
                blocked_processes: vec![],
                return_codes: HashMap::new(),
            };
            kp.sign_job(&mut j);
            j
        };

        let mut ctx = JobExecutionContext {
            data_dir: dir.path(),
            config: &cfg,
            api: &api,
            state: &mut state,
            reboot_mgr: &mut reboot_mgr,
            ws_log_tx: None,
            target_agent_exe: None,
        };

        // 1. force_sync
        let sync_job = make_rc_job("rc-sync-1", "force_sync", "agent", "");
        let r_sync = verify_and_execute_job(&mut ctx, &sync_job).await;
        assert_eq!(r_sync.status, "succeeded");
        assert_eq!(r_sync.exit_code, 0);
        assert_eq!(r_sync.stdout, "Immediate inventory and policy sync triggered.");

        // 2. admin_notify (cap at 10 messages, newest first)
        for i in 0..12 {
            let cmd = serde_json::json!({
                "title": format!("IT Hinweis #{i}"),
                "message": format!("Bitte beachten #{i}"),
                "urgent": i == 11,
            })
            .to_string();
            let notify_job = make_rc_job(&format!("rc-msg-{i}"), "admin_notify", "desktop", &cmd);
            let r_not = verify_and_execute_job(&mut ctx, &notify_job).await;
            assert_eq!(r_not.status, "succeeded");
        }
        assert_eq!(ctx.state.admin_messages.len(), 10);
        assert_eq!(ctx.state.admin_messages[0].id, "rc-msg-11");
        assert!(ctx.state.admin_messages[0].urgent);

        // 3. remote_reboot: grace_period -> cancel -> immediate
        let rb_grace = make_rc_job(
            "rc-rb-grace",
            "remote_reboot",
            "grace_period",
            r#"{"mode":"grace_period","grace_minutes":45}"#,
        );
        let r_grace = verify_and_execute_job(&mut ctx, &rb_grace).await;
        assert_eq!(r_grace.status, "succeeded");
        assert!(r_grace.reboot_required);
        assert!(ctx.state.reboot_state.reboot_required);
        assert_eq!(ctx.state.reboot_state.policy, RebootPolicy::GracePeriod);
        assert_eq!(ctx.state.reboot_state.grace_period_minutes, 45);
        assert!(ctx.state.reboot_state.deadline_unix.is_some());

        let rb_cancel = make_rc_job(
            "rc-rb-cancel",
            "remote_reboot",
            "cancel",
            r#"{"mode":"cancel"}"#,
        );
        let r_cancel = verify_and_execute_job(&mut ctx, &rb_cancel).await;
        assert_eq!(r_cancel.status, "succeeded");
        assert!(!r_cancel.reboot_required);
        assert!(!ctx.state.reboot_state.reboot_required);
        assert!(ctx.state.reboot_state.deadline_unix.is_none());

        let rb_imm = make_rc_job(
            "rc-rb-imm",
            "remote_reboot",
            "immediate",
            r#"{"mode":"immediate"}"#,
        );
        let r_imm = verify_and_execute_job(&mut ctx, &rb_imm).await;
        assert_eq!(r_imm.status, "succeeded");
        assert!(r_imm.reboot_required);
        let marker = std::fs::read_to_string(dir.path().join("last_reboot_triggered.txt")).unwrap();
        assert!(marker.contains("reason=remote_admin_immediate"));
    }
}
