pub mod unix_socket;
pub mod windows_pipe;

use crate::config::{AgentConfig, AgentRuntimeState};
use crate::jobs::runner::{verify_and_execute_job, JobExecutionContext};
use crate::patching::reboot_manager::{execute_system_reboot, RebootManager, RebootPolicyError};
use crate::software::detection::evaluate_detection_rule;
use crate::transport::api_client::AgentApiClient;
use anyhow::{bail, Context, Result};
use chrono::Utc;
use std::collections::{HashMap, HashSet};
use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use swm_common::{
    compare_versions, parse_loopback_tcp_addr, verify_signed_catalog_item, CatalogViewItem,
    CenterStatusSnapshot, IpcErrorCode, IpcRequest, IpcResponse, IpcResponseData,
    JobActivityEntry, PackageAction, WsAgentFrame, AGENT_VERSION, MAX_IPC_FRAME_BYTES,
};
use tokio::io::{AsyncBufReadExt, AsyncRead, AsyncWrite, AsyncWriteExt, BufReader};
use tokio::sync::{mpsc, watch, Mutex};

pub struct AgentIpcInner {
    pub data_dir: PathBuf,
    pub config: AgentConfig,
    pub api: AgentApiClient,
    pub state: AgentRuntimeState,
    pub reboot_mgr: RebootManager,
    pub ws_connected: Arc<AtomicBool>,
    pub ws_log_tx: Option<mpsc::Sender<WsAgentFrame>>,
    pub active_package_status: HashMap<String, (String, Option<String>)>,
    pub active_jobs: Vec<JobActivityEntry>,
    pub claimed_job_ids: HashSet<String>,
}

pub type SharedAgentIpcContext = Arc<Mutex<AgentIpcInner>>;

fn compute_catalog_item_view(
    inner: &AgentIpcInner,
    item: &swm_common::SignedCatalogItem,
) -> CatalogViewItem {
    if let Some((live_state, live_msg)) = inner.active_package_status.get(&item.package_id) {
        return CatalogViewItem {
            package_id: item.package_id.clone(),
            name: item.name.clone(),
            vendor: item.vendor.clone(),
            version: item.version.clone(),
            description: item.description.clone(),
            category: item.category.clone(),
            icon_url: item.icon_url.clone(),
            os_family: item.os_family.clone(),
            source_type: item.source_type.clone(),
            allow_uninstall: item.allow_uninstall,
            installed_version: item.installed_version.clone(),
            status: live_state.clone(),
            blocked_processes: item.blocked_processes.clone(),
            last_message: live_msg.clone(),
        };
    }

    // Determine local installation status via inventory / detection rule / server-provided hint
    let mut detected_ver = item.installed_version.clone();
    if detected_ver.is_none() {
        if let Some(sw) = inner.state.installed_software_cache.iter().find(|s| {
            (!item.package_ref.is_empty()
                && s.package_ref.eq_ignore_ascii_case(&item.package_ref))
                || s.name.eq_ignore_ascii_case(&item.name)
        }) {
            detected_ver = Some(sw.version.clone());
        } else if let Some(rule) = &item.detection_rule {
            if !rule.is_empty()
                && evaluate_detection_rule(rule, &inner.state.installed_software_cache)
            {
                detected_ver = Some(item.version.clone());
            }
        }
    }

    let status = match &detected_ver {
        Some(inst_v) if !inst_v.trim().is_empty() => {
            if !item.version.trim().is_empty()
                && compare_versions(&item.version, inst_v) == std::cmp::Ordering::Greater
            {
                "update_available".to_string()
            } else {
                "installed".to_string()
            }
        }
        _ => {
            if !item.status.trim().is_empty() {
                item.status.clone()
            } else {
                "available".to_string()
            }
        }
    };

    CatalogViewItem {
        package_id: item.package_id.clone(),
        name: item.name.clone(),
        vendor: item.vendor.clone(),
        version: item.version.clone(),
        description: item.description.clone(),
        category: item.category.clone(),
        icon_url: item.icon_url.clone(),
        os_family: item.os_family.clone(),
        source_type: item.source_type.clone(),
        allow_uninstall: item.allow_uninstall,
        installed_version: detected_ver,
        status,
        blocked_processes: item.blocked_processes.clone(),
        last_message: None,
    }
}

/// Processes a single unprivileged `IpcRequest` under strict Ed25519 privilege separation.
pub async fn process_ipc_request(ctx: &SharedAgentIpcContext, req: IpcRequest) -> IpcResponse {
    match req {
        IpcRequest::ListCatalog => {
            let guard = ctx.lock().await;
            let mut verified_views = Vec::new();
            for item in &guard.state.available_catalog {
                // Re-verify Ed25519 signature before exposing or trusting any cached catalog item!
                if verify_signed_catalog_item(
                    item,
                    &guard.config.server_public_key_b64,
                    Some(&guard.config.device_id),
                )
                .is_ok()
                {
                    verified_views.push(compute_catalog_item_view(&guard, item));
                } else {
                    tracing::warn!(
                        package_id = %item.package_id,
                        "Dropped catalog item with invalid Ed25519 signature during ListCatalog"
                    );
                }
            }
            IpcResponse::success(IpcResponseData::Catalog {
                items: verified_views,
            })
        }

        IpcRequest::GetStatus => {
            let guard = ctx.lock().await;
            let snap = CenterStatusSnapshot {
                device_id: guard.config.device_id.clone(),
                server_url: guard.config.server_url.clone(),
                agent_version: AGENT_VERSION.to_string(),
                connected: guard.ws_connected.load(Ordering::SeqCst),
                device_status: guard.config.status.clone(),
                pending_os_updates: guard.state.pending_os_updates_cache.clone(),
                active_jobs: guard.active_jobs.clone(),
                recent_history: guard.state.recent_history.clone(),
                reboot_state: guard.reboot_mgr.state().clone(),
            };
            IpcResponse::success(IpcResponseData::Status(snap))
        }

        IpcRequest::TriggerPackage { package_id, action } => {
            let clean_pkg_id = package_id.trim().to_string();
            if clean_pkg_id.is_empty() {
                return IpcResponse::error(
                    IpcErrorCode::InvalidRequest,
                    "package_id must not be empty",
                );
            }

            // PRIVILEGE SEPARATION GATE:
            // 1. `package_id` MUST exist in swm-agent's cached `available_catalog`.
            // 2. Its Ed25519 server signature MUST verify against `server_public_key_b64` and `device_id`.
            // 3. `Uninstall` MUST be permitted by `allow_uninstall == true` in the signed manifest.
            let (api, pkg_name, pkg_ver) = {
                let mut guard = ctx.lock().await;
                let Some(cat_item) = guard
                    .state
                    .available_catalog
                    .iter()
                    .find(|i| i.package_id == clean_pkg_id)
                    .cloned()
                else {
                    tracing::warn!(
                        package_id = %clean_pkg_id,
                        "SECURITY: Rejected unprivileged IPC TriggerPackage for unknown/unassigned package_id"
                    );
                    return IpcResponse::error(
                        IpcErrorCode::PermissionDenied,
                        format!(
                            "Package '{}' is not authorized in this device's signed catalog",
                            clean_pkg_id
                        ),
                    );
                };

                if let Err(sig_err) = verify_signed_catalog_item(
                    &cat_item,
                    &guard.config.server_public_key_b64,
                    Some(&guard.config.device_id),
                ) {
                    tracing::error!(
                        package_id = %clean_pkg_id,
                        "SECURITY: Rejected IPC TriggerPackage due to tampered catalog signature: {sig_err}"
                    );
                    return IpcResponse::error(
                        IpcErrorCode::PermissionDenied,
                        format!("Catalog signature verification failed: {sig_err}"),
                    );
                }

                if action == PackageAction::Uninstall && !cat_item.allow_uninstall {
                    return IpcResponse::error(
                        IpcErrorCode::PermissionDenied,
                        "Uninstall is disabled by administrator policy for this package",
                    );
                }

                guard
                    .active_package_status
                    .insert(clean_pkg_id.clone(), ("installing".to_string(), None));

                (
                    guard.api.clone(),
                    cat_item.name.clone(),
                    cat_item.version.clone(),
                )
            };

            // Request fresh Ed25519-signed job from server self-service endpoint
            let signed_job = match api
                .trigger_self_service(&clean_pkg_id, action.as_wire())
                .await
            {
                Ok(job) => job,
                Err(err) => {
                    let mut guard = ctx.lock().await;
                    guard.active_package_status.remove(&clean_pkg_id);
                    return IpcResponse::error(
                        IpcErrorCode::AgentOffline,
                        format!("Failed requesting signed job from server: {err}"),
                    );
                }
            };

            // Verify & execute signed job under lock, then report result to server
            let job_res = {
                let mut guard = ctx.lock().await;
                guard.claimed_job_ids.insert(signed_job.job_id.clone());
                guard.active_jobs.push(JobActivityEntry {
                    job_id: signed_job.job_id.clone(),
                    job_type: signed_job.job_type.clone(),
                    action: signed_job.action.clone(),
                    target_name: pkg_name.clone(),
                    version: pkg_ver.clone(),
                    status: "running".to_string(),
                    exit_code: None,
                    reboot_required: false,
                    timestamp: AgentRuntimeState::now_iso8601(),
                    message: "Wird installiert...".to_string(),
                });

                let data_dir = guard.data_dir.clone();
                let config = guard.config.clone();
                let api_ref = guard.api.clone();
                let ws_tx = guard.ws_log_tx.clone();
                let AgentIpcInner {
                    ref mut state,
                    ref mut reboot_mgr,
                    ..
                } = *guard;

                let mut exec_ctx = JobExecutionContext {
                    data_dir: &data_dir,
                    config: &config,
                    api: &api_ref,
                    state,
                    reboot_mgr,
                    ws_log_tx: ws_tx,
                    target_agent_exe: None,
                };
                let outcome = verify_and_execute_job(&mut exec_ctx, &signed_job).await;
                guard.active_jobs.retain(|j| j.job_id != signed_job.job_id);

                match outcome.status.as_str() {
                    "succeeded" => {
                        if action == PackageAction::Uninstall {
                            guard.active_package_status.insert(
                                clean_pkg_id.clone(),
                                ("available".to_string(), None),
                            );
                        } else if outcome.reboot_required {
                            guard.active_package_status.insert(
                                clean_pkg_id.clone(),
                                (
                                    "reboot_required".to_string(),
                                    Some("Neustart ausstehend".to_string()),
                                ),
                            );
                        } else {
                            guard.active_package_status.insert(
                                clean_pkg_id.clone(),
                                ("installed".to_string(), None),
                            );
                        }
                    }
                    "waiting_for_process" => {
                        guard.active_package_status.insert(
                            clean_pkg_id.clone(),
                            (
                                "waiting_for_process".to_string(),
                                Some(outcome.stderr.clone()),
                            ),
                        );
                    }
                    _ => {
                        guard.active_package_status.insert(
                            clean_pkg_id.clone(),
                            ("failed".to_string(), Some(outcome.stderr.clone())),
                        );
                    }
                }

                outcome
            };

            let _ = api.post_job_result(&signed_job.job_id, &job_res).await;
            if job_res.status == "waiting_for_process" {
                ctx.lock().await.claimed_job_ids.remove(&signed_job.job_id);
            }

            if job_res.status == "succeeded" || job_res.status == "waiting_for_process" {
                IpcResponse::success(IpcResponseData::ActionTriggered {
                    job_id: signed_job.job_id,
                    status: job_res.status,
                    message: if !job_res.stderr.is_empty() {
                        job_res.stderr
                    } else {
                        job_res.stdout
                    },
                })
            } else {
                IpcResponse::error(
                    IpcErrorCode::InternalError,
                    format!("Job {} failed: {}", signed_job.job_id, job_res.stderr),
                )
            }
        }

        IpcRequest::TriggerPendingUpdates => {
            let api = { ctx.lock().await.api.clone() };
            let pending = match api.get_pending_jobs().await {
                Ok(p) => p,
                Err(err) => {
                    return IpcResponse::error(
                        IpcErrorCode::AgentOffline,
                        format!("Could not fetch pending updates from server: {err}"),
                    );
                }
            };

            let mut executed = 0usize;
            let mut last_job_id = "none".to_string();
            for job in &pending.jobs {
                if job.job_type == "os_patch" || job.job_type == "software" {
                    let res = {
                        let mut guard = ctx.lock().await;
                        let data_dir = guard.data_dir.clone();
                        let config = guard.config.clone();
                        let api_ref = guard.api.clone();
                        let ws_tx = guard.ws_log_tx.clone();
                        let AgentIpcInner {
                            ref mut state,
                            ref mut reboot_mgr,
                            ..
                        } = *guard;
                        let mut exec_ctx = JobExecutionContext {
                            data_dir: &data_dir,
                            config: &config,
                            api: &api_ref,
                            state,
                            reboot_mgr,
                            ws_log_tx: ws_tx,
                            target_agent_exe: None,
                        };
                        verify_and_execute_job(&mut exec_ctx, job).await
                    };
                    let _ = api.post_job_result(&job.job_id, &res).await;
                    last_job_id = job.job_id.clone();
                    executed += 1;
                }
            }

            IpcResponse::success(IpcResponseData::ActionTriggered {
                job_id: last_job_id,
                status: "succeeded".to_string(),
                message: format!("{executed} ausstehende Update-Aufträge ausgeführt."),
            })
        }

        IpcRequest::SnoozeReboot { minutes } => {
            let mut guard = ctx.lock().await;
            let now_unix = Utc::now().timestamp();
            match guard.reboot_mgr.snooze(minutes, now_unix) {
                Ok(updated) => {
                    guard.state.reboot_state = updated.clone();
                    let _ = guard.state.save(&guard.data_dir);
                    IpcResponse::success(IpcResponseData::RebootUpdated(updated))
                }
                Err(RebootPolicyError::DeadlineExceeded) => IpcResponse::error(
                    IpcErrorCode::DeadlineExceeded,
                    "Die maximale Karenzzeit (Deadline) ist abgelaufen – Neustart kann nicht weiter verschoben werden.",
                ),
                Err(other) => IpcResponse::error(IpcErrorCode::InvalidRequest, other.to_string()),
            }
        }

        IpcRequest::ConfirmRebootNow => {
            let guard = ctx.lock().await;
            if !guard.reboot_mgr.state().reboot_required {
                return IpcResponse::error(
                    IpcErrorCode::InvalidRequest,
                    "Kein ausstehender Systemneustart vorhanden.",
                );
            }
            let data_dir = guard.data_dir.clone();
            let current_state = guard.reboot_mgr.state().clone();
            if let Err(err) = execute_system_reboot(&data_dir, "user_confirmed_reboot_now") {
                return IpcResponse::error(IpcErrorCode::InternalError, err.to_string());
            }
            IpcResponse::success(IpcResponseData::RebootUpdated(current_state))
        }
    }
}

pub async fn handle_ipc_stream<S>(stream: S, ctx: SharedAgentIpcContext) -> Result<()>
where
    S: AsyncRead + AsyncWrite + Unpin,
{
    let (reader, mut writer) = tokio::io::split(stream);
    let mut buf_reader = BufReader::new(reader);
    let mut line = String::new();

    // Enforce bounded frame read
    let bytes_read = buf_reader
        .read_line(&mut line)
        .await
        .context("Failed reading IPC client request")?;
    if bytes_read == 0 {
        return Ok(());
    }

    let response = if line.len() > MAX_IPC_FRAME_BYTES {
        IpcResponse::error(
            IpcErrorCode::InvalidRequest,
            "IPC request frame exceeds maximum permitted size",
        )
    } else {
        match serde_json::from_str::<IpcRequest>(line.trim()) {
            Ok(req) => process_ipc_request(&ctx, req).await,
            Err(err) => IpcResponse::error(
                IpcErrorCode::InvalidRequest,
                format!("Malformed or unsupported IPC command: {err}"),
            ),
        }
    };

    let mut out = serde_json::to_vec(&response)?;
    out.push(b'\n');
    writer.write_all(&out).await?;
    writer.flush().await?;
    Ok(())
}

/// Starts the local privileged-to-user IPC server on Windows Named Pipe, Linux Unix Socket,
/// or strictly loopback TCP (`127.0.0.1:<port>`) for non-admin integration tests.
pub async fn serve_ipc_endpoint(
    ipc_addr: String,
    ctx: SharedAgentIpcContext,
    mut shutdown_rx: watch::Receiver<bool>,
) -> Result<()> {
    if let Some(sock_addr) = parse_loopback_tcp_addr(&ipc_addr)? {
        let listener = tokio::net::TcpListener::bind(sock_addr)
            .await
            .with_context(|| format!("Failed binding loopback IPC listener at {sock_addr}"))?;
        loop {
            tokio::select! {
                _ = shutdown_rx.changed() => {
                    if *shutdown_rx.borrow() {
                        break;
                    }
                }
                accept_res = listener.accept() => {
                    if let Ok((stream, peer)) = accept_res {
                        if !peer.ip().is_loopback() {
                            continue;
                        }
                        let client_ctx = ctx.clone();
                        tokio::spawn(async move {
                            let _ = handle_ipc_stream(stream, client_ctx).await;
                        });
                    }
                }
            }
        }
        return Ok(());
    }

    #[cfg(windows)]
    {
        if ipc_addr.trim().starts_with(r"\\.\pipe\") {
            return windows_pipe::imp::serve_windows_named_pipe(ipc_addr, ctx, shutdown_rx).await;
        }
    }

    #[cfg(unix)]
    {
        if ipc_addr.trim().starts_with('/') || ipc_addr.trim().starts_with("unix:") {
            return unix_socket::imp::serve_unix_domain_socket(ipc_addr, ctx, shutdown_rx).await;
        }
    }

    bail!("Unsupported IPC address '{ipc_addr}' for current OS")
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashMap;
    use swm_common::{send_ipc_request, SignedCatalogItem, TestSigningKeypair};
    use tempfile::tempdir;

    fn build_test_ipc_context(
        dir: &std::path::Path,
        kp: &TestSigningKeypair,
        catalog: Vec<SignedCatalogItem>,
    ) -> SharedAgentIpcContext {
        let cfg = AgentConfig {
            server_url: "http://127.0.0.1:9".to_string(),
            device_id: "dev-ipc-01".to_string(),
            device_token: "secret-device-token-do-not-leak".to_string(),
            status: "approved".to_string(),
            server_public_key_b64: kp.public_key_b64(),
            enrolled_at: "2026-11-20T00:00:00Z".to_string(),
        };
        let api = AgentApiClient::new(&cfg.server_url, Some(cfg.device_token.clone())).unwrap();
        let mut state = AgentRuntimeState::default();
        state.available_catalog = catalog;
        let reboot_mgr = RebootManager::load(dir);

        Arc::new(Mutex::new(AgentIpcInner {
            data_dir: dir.to_path_buf(),
            config: cfg,
            api,
            state,
            reboot_mgr,
            ws_connected: Arc::new(AtomicBool::new(true)),
            ws_log_tx: None,
            active_package_status: HashMap::new(),
            active_jobs: Vec::new(),
            claimed_job_ids: HashSet::new(),
        }))
    }

    #[tokio::test]
    async fn test_ipc_privilege_separation_rejects_unsigned_unknown_and_disallowed_uninstall() {
        let dir = tempdir().unwrap();
        let kp = TestSigningKeypair::generate();

        let mut signed_item = SignedCatalogItem {
            package_id: "pkg-allowed".to_string(),
            device_id: "dev-ipc-01".to_string(),
            name: "7-Zip".to_string(),
            vendor: "Igor Pavlov".to_string(),
            description: "Archiver".to_string(),
            category: "Utilities".to_string(),
            icon_url: String::new(),
            os_family: "windows".to_string(),
            source_type: "winget".to_string(),
            package_ref: "7zip.7zip".to_string(),
            version: "24.08".to_string(),
            sha256: None,
            file_size: 0,
            download_url: None,
            install_cmd: None,
            uninstall_cmd: None,
            allow_uninstall: false,
            installed_version: None,
            status: "available".to_string(),
            blocked_processes: vec![],
            detection_rule: None,
            return_codes: HashMap::new(),
            signature_b64: String::new(),
        };
        kp.sign_catalog_item(&mut signed_item);

        // Also insert a tampered item into cache to verify IPC refuses to trust or list it!
        let mut forged_item = signed_item.clone();
        forged_item.package_id = "pkg-forged".to_string();
        forged_item.install_cmd = Some("cmd.exe /c calc.exe".to_string());

        let ctx = build_test_ipc_context(dir.path(), &kp, vec![signed_item, forged_item]);

        let pipe_or_loopback = if cfg!(windows) {
            format!(r"\\.\pipe\ns8-swm-test-{}", std::process::id())
        } else {
            let l = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
            let a = l.local_addr().unwrap();
            drop(l);
            a.to_string()
        };

        let (shut_tx, shut_rx) = watch::channel(false);
        let srv_addr = pipe_or_loopback.clone();
        let srv_ctx = ctx.clone();
        let srv_handle = tokio::spawn(async move {
            serve_ipc_endpoint(srv_addr, srv_ctx, shut_rx).await.unwrap();
        });
        tokio::time::sleep(std::time::Duration::from_millis(60)).await;

        // 1. ListCatalog returns ONLY the valid Ed25519-signed item (forged item is dropped!)
        let list_resp = send_ipc_request(&pipe_or_loopback, &IpcRequest::ListCatalog)
            .await
            .unwrap();
        assert!(list_resp.ok);
        match list_resp.data.unwrap() {
            IpcResponseData::Catalog { items } => {
                assert_eq!(items.len(), 1);
                assert_eq!(items[0].package_id, "pkg-allowed");
            }
            other => panic!("Unexpected data: {other:?}"),
        }

        // 2. TriggerPackage with unknown package_id -> PermissionDenied
        let unknown_resp = send_ipc_request(
            &pipe_or_loopback,
            &IpcRequest::TriggerPackage {
                package_id: "pkg-not-in-catalog".to_string(),
                action: PackageAction::Install,
            },
        )
        .await
        .unwrap();
        assert!(!unknown_resp.ok);
        assert_eq!(unknown_resp.error_code, Some(IpcErrorCode::PermissionDenied));

        // 3. TriggerPackage with forged signature item -> PermissionDenied
        let forged_resp = send_ipc_request(
            &pipe_or_loopback,
            &IpcRequest::TriggerPackage {
                package_id: "pkg-forged".to_string(),
                action: PackageAction::Install,
            },
        )
        .await
        .unwrap();
        assert!(!forged_resp.ok);
        assert_eq!(forged_resp.error_code, Some(IpcErrorCode::PermissionDenied));

        // 4. TriggerPackage Uninstall when allow_uninstall == false -> PermissionDenied
        let uninst_resp = send_ipc_request(
            &pipe_or_loopback,
            &IpcRequest::TriggerPackage {
                package_id: "pkg-allowed".to_string(),
                action: PackageAction::Uninstall,
            },
        )
        .await
        .unwrap();
        assert!(!uninst_resp.ok);
        assert_eq!(uninst_resp.error_code, Some(IpcErrorCode::PermissionDenied));

        // 5. GetStatus never leaks device_token
        let status_resp = send_ipc_request(&pipe_or_loopback, &IpcRequest::GetStatus)
            .await
            .unwrap();
        let raw_json = serde_json::to_string(&status_resp).unwrap();
        assert!(!raw_json.contains("secret-device-token-do-not-leak"));

        // 6. Non-loopback TCP IPC address is rejected by security check
        assert!(parse_loopback_tcp_addr("0.0.0.0:9999").is_err());

        let _ = shut_tx.send(true);
        let _ = srv_handle.await;
    }
}
