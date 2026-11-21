use axum::body::Body;
use axum::extract::{Path as AxumPath, State};
use axum::http::{HeaderMap, StatusCode};
use axum::response::Response;
use axum::routing::{get, post};
use axum::{Json, Router};
use chrono::Utc;
use std::collections::HashMap;
use std::sync::{Arc, Mutex};
use swm_agent::daemon::{enroll_device, run_daemon_loop, run_once, CycleOutcome};
use swm_common::{
    compute_bytes_sha256, send_ipc_request, AgentStatusResponse, EnrollRequest, EnrollResponse,
    InventoryAckResponse, InventoryReport, IpcErrorCode, IpcRequest, IpcResponseData, JobResult,
    PackageAction, PackageMeta, PendingJobsResponse, SelfServiceTriggerRequest, SignedCatalogItem,
    SignedJob, TestSigningKeypair,
};
use tempfile::tempdir;
use tokio::sync::watch;

struct MockServerData {
    approved: bool,
    inventory_received: usize,
    pending_jobs: Vec<SignedJob>,
    available_catalog: Vec<SignedCatalogItem>,
    job_results: HashMap<String, JobResult>,
    package_bytes: Vec<u8>,
    package_sha256: String,
    package_filename: String,
    kp: TestSigningKeypair,
}

#[derive(Clone)]
struct MockServerState(Arc<Mutex<MockServerData>>);

async fn handle_enroll(
    State(st): State<MockServerState>,
    Json(req): Json<EnrollRequest>,
) -> Json<EnrollResponse> {
    assert_eq!(req.enrollment_token, "swm_tok_valid");
    let guard = st.0.lock().unwrap();
    Json(EnrollResponse {
        device_id: "dev-e2e-1".to_string(),
        device_token: "swm_dev_tok_e2e".to_string(),
        status: "pending".to_string(),
        server_public_key_b64: guard.kp.public_key_b64(),
    })
}

async fn handle_status(State(st): State<MockServerState>) -> Json<AgentStatusResponse> {
    let guard = st.0.lock().unwrap();
    Json(AgentStatusResponse {
        device_id: "dev-e2e-1".to_string(),
        status: if guard.approved {
            "approved".to_string()
        } else {
            "pending".to_string()
        },
        reboot_policy: "grace_period".to_string(),
        grace_period_minutes: 180,
        in_maintenance_window: true,
        excluded_updates: vec!["KB5034441".to_string()],
        server_public_key_b64: Some(guard.kp.public_key_b64()),
    })
}

async fn handle_inventory(
    State(st): State<MockServerState>,
    Json(inv): Json<InventoryReport>,
) -> Json<InventoryAckResponse> {
    assert!(!inv.hostname.is_empty());
    let mut guard = st.0.lock().unwrap();
    guard.inventory_received += 1;
    Json(InventoryAckResponse {
        status: "ok".to_string(),
        enqueued_jobs: guard.pending_jobs.len() as u32,
        software_updated: inv.installed_software.is_some(),
    })
}

async fn handle_pending(State(st): State<MockServerState>) -> Json<PendingJobsResponse> {
    let mut guard = st.0.lock().unwrap();
    let jobs = std::mem::take(&mut guard.pending_jobs);
    Json(PendingJobsResponse {
        jobs,
        available_catalog: guard.available_catalog.clone(),
    })
}

async fn handle_job_result(
    State(st): State<MockServerState>,
    AxumPath(job_id): AxumPath<String>,
    Json(res): Json<JobResult>,
) -> Json<serde_json::Value> {
    let mut guard = st.0.lock().unwrap();
    guard.job_results.insert(job_id.clone(), res.clone());
    Json(serde_json::json!({
        "status": "ok",
        "job_id": job_id,
        "job_status": res.status,
    }))
}

async fn handle_download(State(st): State<MockServerState>, headers: HeaderMap) -> Response<Body> {
    let guard = st.0.lock().unwrap();
    let total = guard.package_bytes.len();
    if let Some(r) = headers.get("range").and_then(|v| v.to_str().ok()) {
        if let Some(start_s) = r.strip_prefix("bytes=").and_then(|s| s.split('-').next()) {
            if let Ok(start) = start_s.parse::<usize>() {
                if start < total {
                    return Response::builder()
                        .status(StatusCode::PARTIAL_CONTENT)
                        .header("Accept-Ranges", "bytes")
                        .header("X-Package-SHA256", &guard.package_sha256)
                        .body(Body::from(guard.package_bytes[start..].to_vec()))
                        .unwrap();
                }
            }
        }
    }
    Response::builder()
        .status(StatusCode::OK)
        .header("Accept-Ranges", "bytes")
        .header("X-Package-SHA256", &guard.package_sha256)
        .body(Body::from(guard.package_bytes.clone()))
        .unwrap()
}

async fn handle_self_service_trigger(
    State(st): State<MockServerState>,
    Json(req): Json<SelfServiceTriggerRequest>,
) -> Json<SignedJob> {
    let guard = st.0.lock().unwrap();
    let mut job = SignedJob {
        job_id: "job-self-service-1".to_string(),
        device_id: "dev-e2e-1".to_string(),
        job_type: "software".to_string(),
        action: req.action,
        target_id: req.package_id.clone(),
        version: "2.0.0".to_string(),
        sha256: Some(guard.package_sha256.clone()),
        command: Some(String::new()),
        expires_at_unix: Utc::now().timestamp() + 3600,
        signature_b64: String::new(),
        download_url: Some(format!("/api/v1/agent/packages/{}/download", req.package_id)),
        update_ids: vec![],
        security_only: false,
        package_meta: Some(PackageMeta {
            package_id: req.package_id,
            name: "SelfServiceTool".to_string(),
            source_type: "custom".to_string(),
            filename: guard.package_filename.clone(),
            package_ref: guard.package_filename.clone(),
            ..Default::default()
        }),
        detection_rule: None,
        blocked_processes: vec![],
        return_codes: HashMap::from([("100".to_string(), "reboot_required".to_string())]),
    };
    guard.kp.sign_job(&mut job);
    Json(job)
}

#[tokio::test]
async fn test_full_agent_enroll_approval_signed_job_and_ipc_self_service_e2e() {
    let dir = tempdir().unwrap();
    let kp = TestSigningKeypair::generate();

    let (pkg_filename, script_bytes, expected_exit_code) = if cfg!(windows) {
        (
            "install_tool.cmd".to_string(),
            b"@echo off\r\necho INSTALLED_OK\r\nexit /b 3010\r\n".to_vec(),
            3010,
        )
    } else {
        (
            "install_tool.sh".to_string(),
            b"#!/bin/sh\necho INSTALLED_OK\nexit 100\n".to_vec(),
            100,
        )
    };
    let pkg_sha = compute_bytes_sha256(&script_bytes);

    let custom_return_codes =
        HashMap::from([("100".to_string(), "reboot_required".to_string())]);

    let mut req_job = SignedJob {
        job_id: "job-req-1".to_string(),
        device_id: "dev-e2e-1".to_string(),
        job_type: "software".to_string(),
        action: "install".to_string(),
        target_id: "pkg-custom-1".to_string(),
        version: "1.0.0".to_string(),
        sha256: Some(pkg_sha.clone()),
        command: Some(String::new()),
        expires_at_unix: Utc::now().timestamp() + 3600,
        signature_b64: String::new(),
        download_url: Some("/api/v1/agent/packages/pkg-custom-1/download".to_string()),
        update_ids: vec![],
        security_only: false,
        package_meta: Some(PackageMeta {
            package_id: "pkg-custom-1".to_string(),
            name: "CorporateTool".to_string(),
            source_type: "custom".to_string(),
            filename: pkg_filename.clone(),
            package_ref: pkg_filename.clone(),
            ..Default::default()
        }),
        detection_rule: None,
        blocked_processes: vec![],
        return_codes: custom_return_codes.clone(),
    };
    kp.sign_job(&mut req_job);

    let mut avail_item = SignedCatalogItem {
        package_id: "pkg-avail-2".to_string(),
        device_id: "dev-e2e-1".to_string(),
        name: "SelfServiceTool".to_string(),
        vendor: "Internal IT".to_string(),
        description: "Self-service desktop utility".to_string(),
        category: "Productivity".to_string(),
        icon_url: String::new(),
        os_family: if cfg!(windows) { "windows" } else { "linux" }.to_string(),
        source_type: "custom".to_string(),
        package_ref: pkg_filename.clone(),
        version: "2.0.0".to_string(),
        sha256: Some(pkg_sha.clone()),
        file_size: script_bytes.len() as u64,
        download_url: Some("/api/v1/agent/packages/pkg-avail-2/download".to_string()),
        install_cmd: Some(String::new()),
        uninstall_cmd: Some(String::new()),
        allow_uninstall: true,
        installed_version: None,
        status: "available".to_string(),
        blocked_processes: vec![],
        detection_rule: None,
        return_codes: HashMap::new(),
        signature_b64: String::new(),
    };
    kp.sign_catalog_item(&mut avail_item);

    let state = MockServerState(Arc::new(Mutex::new(MockServerData {
        approved: false,
        inventory_received: 0,
        pending_jobs: vec![req_job],
        available_catalog: vec![avail_item],
        job_results: HashMap::new(),
        package_bytes: script_bytes,
        package_sha256: pkg_sha,
        package_filename: pkg_filename,
        kp,
    })));

    let app = Router::new()
        .route("/api/v1/agent/enroll", post(handle_enroll))
        .route("/api/v1/agent/status", get(handle_status))
        .route("/api/v1/agent/inventory", post(handle_inventory))
        .route("/api/v1/agent/jobs/pending", get(handle_pending))
        .route("/api/v1/agent/jobs/:job_id/result", post(handle_job_result))
        .route(
            "/api/v1/agent/packages/:pkg_id/download",
            get(handle_download),
        )
        .route(
            "/api/v1/agent/self-service/trigger",
            post(handle_self_service_trigger),
        )
        .with_state(state.clone());

    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let srv_addr = listener.local_addr().unwrap();
    let srv_url = format!("http://{srv_addr}");
    tokio::spawn(async move {
        axum::serve(listener, app).await.unwrap();
    });

    // 1. Enroll -> starts in `pending`
    let enrolled = enroll_device(dir.path(), &srv_url, "swm_tok_valid", false)
        .await
        .unwrap();
    assert_eq!(enrolled.status, "pending");

    // 2. While pending, run_once cleanly reports PendingApproval without posting inventory
    let pending_cycle = run_once(dir.path().to_path_buf()).await.unwrap();
    assert_eq!(pending_cycle, CycleOutcome::PendingApproval);
    assert_eq!(state.0.lock().unwrap().inventory_received, 0);

    // 3. Admin approves device -> run_once submits inventory, executes required job, maps 3010 reboot!
    state.0.lock().unwrap().approved = true;
    let approved_cycle = run_once(dir.path().to_path_buf()).await.unwrap();
    assert_eq!(approved_cycle, CycleOutcome::Completed { jobs_executed: 1 });

    {
        let guard = state.0.lock().unwrap();
        assert!(guard.inventory_received >= 1);
        let r1 = guard.job_results.get("job-req-1").unwrap();
        assert_eq!(r1.status, "succeeded");
        assert_eq!(r1.exit_code, expected_exit_code);
        assert!(r1.reboot_required);
        assert!(r1.stdout.contains("INSTALLED_OK"));
    }

    // 4. Start daemon with local IPC and trigger self-service installation + reboot snooze
    let ipc_probe = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let ipc_addr = ipc_probe.local_addr().unwrap().to_string();
    drop(ipc_probe);

    let (shut_tx, shut_rx) = watch::channel(false);
    let daemon_dir = dir.path().to_path_buf();
    let daemon_ipc = ipc_addr.clone();
    let daemon_handle = tokio::spawn(async move {
        run_daemon_loop(daemon_dir, daemon_ipc, 30, shut_rx)
            .await
            .unwrap();
    });
    tokio::time::sleep(std::time::Duration::from_millis(150)).await;

    // Trigger authorized package -> succeeds & reports result to server
    let trig_ok = send_ipc_request(
        &ipc_addr,
        &IpcRequest::TriggerPackage {
            package_id: "pkg-avail-2".to_string(),
            action: PackageAction::Install,
        },
    )
    .await
    .unwrap();
    assert!(trig_ok.ok);

    // Trigger unauthorized package -> strictly rejected with PermissionDenied
    let trig_denied = send_ipc_request(
        &ipc_addr,
        &IpcRequest::TriggerPackage {
            package_id: "pkg-evil".to_string(),
            action: PackageAction::Install,
        },
    )
    .await
    .unwrap();
    assert!(!trig_denied.ok);
    assert_eq!(trig_denied.error_code, Some(IpcErrorCode::PermissionDenied));

    // Snooze grace-period reboot via IPC -> succeeds
    let snooze_ok = send_ipc_request(&ipc_addr, &IpcRequest::SnoozeReboot { minutes: 60 })
        .await
        .unwrap();
    assert!(snooze_ok.ok);
    match snooze_ok.data.unwrap() {
        IpcResponseData::RebootUpdated(rb) => {
            assert!(rb.reboot_required);
            assert!(rb.snoozed_until_unix.is_some());
        }
        other => panic!("Unexpected IPC data: {other:?}"),
    }

    assert!(state
        .0
        .lock()
        .unwrap()
        .job_results
        .contains_key("job-self-service-1"));

    let _ = shut_tx.send(true);
    let _ = daemon_handle.await;
}
