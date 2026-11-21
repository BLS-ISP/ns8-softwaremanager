use crate::config::{normalize_server_url, AgentConfig, AgentRuntimeState};
use crate::inventory::reboot::detect_os_pending_reboot;
use crate::inventory::system::collect_system_snapshot;
use crate::inventory::build_inventory_report;
use crate::ipc::{serve_ipc_endpoint, AgentIpcInner, SharedAgentIpcContext};
use crate::jobs::runner::{verify_and_execute_job, JobExecutionContext};
use crate::patching::linux_pkg::scan_linux_os_updates;
use crate::patching::reboot_manager::{execute_system_reboot, RebootManager};
use crate::patching::windows_wua::{PowerShellWuaProvider, WuaProvider};
use crate::transport::api_client::{AgentApiClient, DeviceAuthStatus};
use crate::transport::ws_client::{run_ws_supervisor, WsServerSignal};
use anyhow::{bail, Context, Result};
use chrono::Utc;
use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::time::Duration;
use swm_common::{
    decode_public_key_b64, verify_signed_catalog_item, EnrollRequest, RebootPolicy, WsAgentFrame,
    AGENT_VERSION,
};
use tokio::sync::{mpsc, watch, Mutex};

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CycleOutcome {
    PendingApproval,
    Revoked,
    Completed { jobs_executed: usize },
}

/// Registers this device with the central NS8-SoftwareManager server and persists `config.json`.
pub async fn enroll_device(
    data_dir: &Path,
    server_url: &str,
    enrollment_token: &str,
    wait_for_approval: bool,
) -> Result<AgentConfig> {
    let clean_url = normalize_server_url(server_url);
    let api = AgentApiClient::new(&clean_url, None)?;
    let snap = collect_system_snapshot();

    let req = EnrollRequest {
        enrollment_token: enrollment_token.trim().to_string(),
        hostname: snap.hostname,
        os_family: snap.os_family,
        os_name: snap.os_name,
        os_version: snap.os_version,
        arch: snap.arch,
        agent_version: AGENT_VERSION.to_string(),
        machine_uid: snap.machine_uid,
    };

    let resp = api.enroll(&req).await?;

    // Validate Ed25519 public key format before trusting or saving
    decode_public_key_b64(&resp.server_public_key_b64)
        .context("Server returned an invalid Ed25519 public key during enrollment")?;

    let mut cfg = AgentConfig {
        server_url: clean_url.clone(),
        device_id: resp.device_id,
        device_token: resp.device_token,
        status: resp.status,
        server_public_key_b64: resp.server_public_key_b64,
        enrolled_at: AgentRuntimeState::now_iso8601(),
    };
    cfg.save(data_dir)?;

    if wait_for_approval && cfg.status == "pending" {
        let authed_api = AgentApiClient::new(&clean_url, Some(cfg.device_token.clone()))?;
        loop {
            tokio::time::sleep(Duration::from_secs(3)).await;
            match authed_api.get_status().await? {
                DeviceAuthStatus::Approved(_) => {
                    cfg.status = "approved".to_string();
                    cfg.save(data_dir)?;
                    break;
                }
                DeviceAuthStatus::Pending(_) => continue,
                DeviceAuthStatus::Revoked => {
                    cfg.status = "revoked".to_string();
                    cfg.save(data_dir)?;
                    bail!("Device enrollment was revoked by administrator");
                }
            }
        }
    }

    Ok(cfg)
}

pub fn build_shared_context(
    data_dir: PathBuf,
    config: AgentConfig,
    ws_connected: Arc<AtomicBool>,
    ws_log_tx: Option<mpsc::Sender<WsAgentFrame>>,
) -> Result<SharedAgentIpcContext> {
    let api = AgentApiClient::new(&config.server_url, Some(config.device_token.clone()))?;
    let state = AgentRuntimeState::load_or_default(&data_dir);
    let reboot_mgr = RebootManager::load(&data_dir);

    Ok(Arc::new(Mutex::new(AgentIpcInner {
        data_dir,
        config,
        api,
        state,
        reboot_mgr,
        ws_connected,
        ws_log_tx,
        active_package_status: HashMap::new(),
        active_jobs: Vec::new(),
        claimed_job_ids: HashSet::new(),
    })))
}

/// Runs one full deterministic synchronization cycle:
/// `GET /status` -> `POST /inventory` -> `GET /jobs/pending` (catalog signature verification + sequential signed job execution) -> `POST /jobs/{id}/result`.
pub async fn run_sync_cycle(
    shared: &SharedAgentIpcContext,
    force_full_software: bool,
    passive_os_scan: bool,
    reboot_flag: &AtomicBool,
) -> Result<CycleOutcome> {
    let api = { shared.lock().await.api.clone() };

    // 1. Check device approval / revocation & group policies
    let status_resp = match api.get_status().await? {
        DeviceAuthStatus::Revoked => {
            let mut guard = shared.lock().await;
            guard.config.status = "revoked".to_string();
            let _ = guard.config.save(&guard.data_dir);
            return Ok(CycleOutcome::Revoked);
        }
        DeviceAuthStatus::Pending(_) => {
            let mut guard = shared.lock().await;
            guard.config.status = "pending".to_string();
            let _ = guard.config.save(&guard.data_dir);
            return Ok(CycleOutcome::PendingApproval);
        }
        DeviceAuthStatus::Approved(st) => st,
    };

    let excluded_updates = status_resp.excluded_updates.clone();
    let in_maint_window = status_resp.in_maintenance_window;

    {
        let mut guard = shared.lock().await;
        if guard.config.status != "approved" {
            guard.config.status = "approved".to_string();
            let _ = guard.config.save(&guard.data_dir);
        }
        guard.state.excluded_updates = excluded_updates.clone();
        guard.state.in_maintenance_window = in_maint_window;

        let os_pending = detect_os_pending_reboot();
        let policy = RebootPolicy::from_wire(&status_resp.reboot_policy);
        let _ = guard.reboot_mgr.sync_policy_and_os_state(
            policy,
            status_resp.grace_period_minutes,
            os_pending,
            Utc::now().timestamp(),
        );
        guard.state.reboot_state = guard.reboot_mgr.state().clone();
        reboot_flag.store(guard.state.reboot_state.reboot_required, Ordering::SeqCst);
    }

    // 2. Collect OS pending updates if enabled or mocked, then submit inventory
    let pending_os_updates = if passive_os_scan
        || std::env::var("SWM_MOCK_WUA_JSON").is_ok()
        || std::env::var("SWM_MOCK_LINUX_UPDATES_JSON").is_ok()
    {
        if cfg!(windows) {
            PowerShellWuaProvider
                .scan(&excluded_updates, Duration::from_secs(8))
                .await
                .unwrap_or_default()
        } else {
            scan_linux_os_updates(&excluded_updates)
                .await
                .unwrap_or_default()
        }
    } else {
        shared.lock().await.state.pending_os_updates_cache.clone()
    };

    let (prev_hash, extra_reboot) = {
        let guard = shared.lock().await;
        (
            guard.state.last_software_hash.clone(),
            guard.reboot_mgr.state().reboot_required,
        )
    };

    let collected = build_inventory_report(
        prev_hash.as_deref(),
        force_full_software,
        pending_os_updates.clone(),
        extra_reboot,
    );

    let inv_ack = api.post_inventory(&collected.report).await?;
    {
        let mut guard = shared.lock().await;
        guard.state.last_software_hash = Some(collected.current_software_hash);
        guard.state.installed_software_cache = collected.all_software;
        guard.state.pending_os_updates_cache = pending_os_updates;
        reboot_flag.store(collected.report.reboot_required, Ordering::SeqCst);
        let _ = guard.state.save(&guard.data_dir);
    }

    // 3. Pull pending signed jobs & signed self-service catalog
    let pending_bundle = api.get_pending_jobs().await?;

    {
        let mut guard = shared.lock().await;
        let mut verified_catalog = Vec::new();
        for item in pending_bundle.available_catalog {
            match verify_signed_catalog_item(
                &item,
                &guard.config.server_public_key_b64,
                Some(&guard.config.device_id),
            ) {
                Ok(()) => verified_catalog.push(item),
                Err(err) => {
                    tracing::error!(
                        package_id = %item.package_id,
                        "SECURITY: Dropped available_catalog item with invalid signature: {err}"
                    );
                }
            }
        }
        guard.state.available_catalog = verified_catalog;
        let _ = guard.state.save(&guard.data_dir);
    }

    // 4. Execute each signed job sequentially and report result
    let mut jobs_executed = 0usize;
    let mut software_mutated = false;

    for job in &pending_bundle.jobs {
        let should_run = {
            let mut guard = shared.lock().await;
            guard.claimed_job_ids.insert(job.job_id.clone())
        };
        if !should_run {
            continue;
        }

        let job_res = {
            let mut guard = shared.lock().await;
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

        api.post_job_result(&job.job_id, &job_res).await?;
        if job_res.status == "waiting_for_process" {
            shared.lock().await.claimed_job_ids.remove(&job.job_id);
        }
        if job.job_type == "software" && job_res.status == "succeeded" {
            software_mutated = true;
        }
        if job_res.reboot_required {
            reboot_flag.store(true, Ordering::SeqCst);
        }
        jobs_executed += 1;
    }

    // If software was installed/removed and OS software registry/database changed, refresh inventory snapshot
    if software_mutated {
        let prev_sw_hash = shared.lock().await.state.last_software_hash.clone();
        let refreshed = build_inventory_report(
            None,
            true,
            shared.lock().await.state.pending_os_updates_cache.clone(),
            reboot_flag.load(Ordering::SeqCst),
        );
        if prev_sw_hash.as_deref() != Some(refreshed.current_software_hash.as_str()) {
            let _ = api.post_inventory(&refreshed.report).await;
            let mut guard = shared.lock().await;
            guard.state.last_software_hash = Some(refreshed.current_software_hash);
            guard.state.installed_software_cache = refreshed.all_software;
            let _ = guard.state.save(&guard.data_dir);
        }
    }
    let _ = inv_ack;

    // 5. Evaluate Reboot Policy Manager for automatic reboot eligibility
    {
        let guard = shared.lock().await;
        if guard
            .reboot_mgr
            .should_execute_auto_reboot(Utc::now().timestamp(), guard.state.in_maintenance_window)
        {
            let _ = execute_system_reboot(&guard.data_dir, "reboot_policy_scheduled");
        }
    }

    Ok(CycleOutcome::Completed { jobs_executed })
}

/// Runs a single `--once` synchronization cycle (ideal for CLI & deterministic E2E testing).
pub async fn run_once(data_dir: PathBuf) -> Result<CycleOutcome> {
    let config = AgentConfig::load(&data_dir)?;
    let ws_connected = Arc::new(AtomicBool::new(false));
    let reboot_flag = AtomicBool::new(false);
    let shared = build_shared_context(data_dir, config, ws_connected, None)?;

    let passive_scan = std::env::var("SWM_PASSIVE_OS_SCAN")
        .ok()
        .map(|v| v == "1")
        .unwrap_or(false);

    run_sync_cycle(&shared, true, passive_scan, &reboot_flag).await
}

/// Runs the full privileged background daemon:
/// - Local IPC server (Windows Named Pipe / Unix Socket / loopback test socket)
/// - Pending-approval wait loop
/// - Persistent outbound WebSocket (`WS /api/v1/agent/ws`) + HTTPS polling fallback
pub async fn run_daemon_loop(
    data_dir: PathBuf,
    ipc_addr: String,
    poll_interval_secs: u64,
    mut shutdown_rx: watch::Receiver<bool>,
) -> Result<()> {
    let config = AgentConfig::load(&data_dir)?;
    let ws_connected = Arc::new(AtomicBool::new(false));
    let reboot_flag = Arc::new(AtomicBool::new(false));
    let (ws_log_tx, ws_log_rx) = mpsc::channel::<WsAgentFrame>(128);
    let (sig_tx, mut sig_rx) = mpsc::channel::<WsServerSignal>(32);

    let shared = build_shared_context(
        data_dir.clone(),
        config.clone(),
        ws_connected.clone(),
        Some(ws_log_tx),
    )?;

    // Start local IPC server for unprivileged Software Center (`swm-center`)
    let ipc_ctx = shared.clone();
    let ipc_shutdown = shutdown_rx.clone();
    let ipc_addr_clone = ipc_addr.clone();
    let ipc_task = tokio::spawn(async move {
        if let Err(err) = serve_ipc_endpoint(ipc_addr_clone, ipc_ctx, ipc_shutdown).await {
            tracing::error!("Local IPC server exited with error: {err}");
        }
    });

    // Wait while device is in `pending` approval state before opening WS
    loop {
        if *shutdown_rx.borrow() {
            let _ = ipc_task.await;
            return Ok(());
        }

        match run_sync_cycle(&shared, true, false, &reboot_flag).await {
            Ok(CycleOutcome::PendingApproval) => {
                tracing::info!("Device is awaiting administrator approval (status=pending)...");
                tokio::select! {
                    _ = tokio::time::sleep(Duration::from_secs(poll_interval_secs.max(2))) => continue,
                    _ = shutdown_rx.changed() => {
                        if *shutdown_rx.borrow() {
                            return Ok(());
                        }
                    }
                }
            }
            Ok(CycleOutcome::Revoked) => {
                tracing::error!("Device has been revoked by administrator; shutting down daemon.");
                return Ok(());
            }
            Ok(CycleOutcome::Completed { .. }) => break,
            Err(err) => {
                tracing::warn!("Initial sync failed ({err}), retrying...");
                tokio::select! {
                    _ = tokio::time::sleep(Duration::from_secs(5)) => continue,
                    _ = shutdown_rx.changed() => {
                        if *shutdown_rx.borrow() {
                            return Ok(());
                        }
                    }
                }
            }
        }
    }

    // Launch persistent WebSocket supervisor
    let ws_task = tokio::spawn(run_ws_supervisor(
        config.server_url.clone(),
        config.device_token.clone(),
        ws_connected.clone(),
        reboot_flag.clone(),
        ws_log_rx,
        sig_tx,
        shutdown_rx.clone(),
    ));

    let mut fallback_ticker =
        tokio::time::interval(Duration::from_secs(poll_interval_secs.max(5)));
    fallback_ticker.tick().await;

    loop {
        tokio::select! {
            _ = shutdown_rx.changed() => {
                if *shutdown_rx.borrow() {
                    break;
                }
            }
            Some(sig) = sig_rx.recv() => {
                match sig {
                    WsServerSignal::Revoked => {
                        let mut guard = shared.lock().await;
                        guard.config.status = "revoked".to_string();
                        let _ = guard.config.save(&guard.data_dir);
                        tracing::error!("Received WebSocket revocation signal; shutting down.");
                        break;
                    }
                    WsServerSignal::JobAvailable(_) | WsServerSignal::CatalogUpdated => {
                        if let Ok(CycleOutcome::Revoked) = run_sync_cycle(&shared, false, false, &reboot_flag).await {
                            break;
                        }
                    }
                }
            }
            _ = fallback_ticker.tick() => {
                match run_sync_cycle(&shared, false, false, &reboot_flag).await {
                    Ok(CycleOutcome::Revoked) => break,
                    Ok(_) => {}
                    Err(err) => tracing::warn!("Periodic sync cycle error: {err}"),
                }
            }
        }
    }

    ws_task.abort();
    ipc_task.abort();
    Ok(())
}
