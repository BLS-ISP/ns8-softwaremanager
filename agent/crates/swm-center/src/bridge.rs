use axum::body::Body;
use axum::extract::{Path as AxumPath, State};
use axum::http::{header, StatusCode};
use axum::response::{IntoResponse, Response};
use axum::routing::{get, post};
use axum::{Json, Router};
use serde::{Deserialize, Serialize};
use std::fs;
use std::net::SocketAddr;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Arc;
use std::time::Duration;
use swm_common::{send_ipc_request, IpcErrorCode, IpcRequest, IpcResponse, PackageAction};
use tokio::io::{AsyncReadExt, AsyncWriteExt};

include!(concat!(env!("OUT_DIR"), "/embedded_ui.rs"));

#[derive(Clone, Default)]
pub struct CenterBridgeState {
    pub ipc_addr: String,
    pub ui_dist_dir: Option<PathBuf>,
    pub local_url: String,
    pub no_open: bool,
    pub open_window_count: Arc<AtomicUsize>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct CenterSessionInfo {
    pub url: String,
}

#[derive(Debug, Deserialize)]
pub struct TriggerPayload {
    pub package_id: String,
    pub action: PackageAction,
}

#[derive(Debug, Deserialize)]
pub struct SnoozePayload {
    pub minutes: u32,
}

#[derive(Debug, Deserialize)]
pub struct DismissMessagePayload {
    pub message_id: String,
}

/// Resolves the per-user Single-Instance session file location:
/// - Override via `SWM_CENTER_SESSION_FILE` (for isolated tests)
/// - Windows: `%LOCALAPPDATA%\NS8-SoftwareManager\center-session.json`
/// - Unix: `$XDG_RUNTIME_DIR/ns8-swm-center.json` or `/tmp/ns8-swm-center-<uid>.json`
pub fn default_session_file_path() -> PathBuf {
    if let Ok(override_path) = std::env::var("SWM_CENTER_SESSION_FILE") {
        let trimmed = override_path.trim();
        if !trimmed.is_empty() {
            return PathBuf::from(trimmed);
        }
    }

    #[cfg(windows)]
    {
        if let Ok(local_app_data) = std::env::var("LOCALAPPDATA") {
            if !local_app_data.trim().is_empty() {
                return PathBuf::from(local_app_data)
                    .join("NS8-SoftwareManager")
                    .join("center-session.json");
            }
        }
        if let Ok(profile) = std::env::var("USERPROFILE") {
            if !profile.trim().is_empty() {
                return PathBuf::from(profile)
                    .join("AppData")
                    .join("Local")
                    .join("NS8-SoftwareManager")
                    .join("center-session.json");
            }
        }
        std::env::temp_dir()
            .join("NS8-SoftwareManager")
            .join("center-session.json")
    }

    #[cfg(not(windows))]
    {
        if let Ok(xdg) = std::env::var("XDG_RUNTIME_DIR") {
            if !xdg.trim().is_empty() {
                return PathBuf::from(xdg).join("ns8-swm-center.json");
            }
        }
        let uid = detect_unix_uid();
        PathBuf::from(format!("/tmp/ns8-swm-center-{uid}.json"))
    }
}

#[cfg(not(windows))]
fn detect_unix_uid() -> String {
    if let Ok(status) = fs::read_to_string("/proc/self/status") {
        for line in status.lines() {
            if let Some(rest) = line.strip_prefix("Uid:") {
                if let Some(uid_tok) = rest.split_whitespace().next() {
                    if !uid_tok.is_empty() && uid_tok.chars().all(|c| c.is_ascii_digit()) {
                        return uid_tok.to_string();
                    }
                }
            }
        }
    }
    std::env::var("UID")
        .ok()
        .filter(|s| !s.trim().is_empty())
        .or_else(|| std::env::var("USER").ok().filter(|s| !s.trim().is_empty()))
        .unwrap_or_else(|| "0".to_string())
}

fn parse_session_loopback_addr(url: &str) -> Option<SocketAddr> {
    let trimmed = url.trim().trim_end_matches('/');
    let host_port = trimmed.strip_prefix("http://")?;
    let host_port = host_port.split('/').next()?;
    let addr: SocketAddr = host_port.parse().ok()?;
    if addr.ip().is_loopback() && addr.port() > 0 {
        Some(addr)
    } else {
        None
    }
}

pub async fn probe_existing_center(url: &str, timeout: Duration) -> bool {
    let Some(addr) = parse_session_loopback_addr(url) else {
        return false;
    };
    let probe = async move {
        let mut stream = tokio::net::TcpStream::connect(addr).await.ok()?;
        let req = format!(
            "GET /api/ping HTTP/1.1\r\nHost: {addr}\r\nConnection: close\r\n\r\n"
        );
        stream.write_all(req.as_bytes()).await.ok()?;
        let mut buf = Vec::with_capacity(512);
        stream.read_to_end(&mut buf).await.ok()?;
        let raw = String::from_utf8_lossy(&buf);
        let (head, body) = raw.split_once("\r\n\r\n")?;
        let status_code = head
            .lines()
            .next()?
            .split_whitespace()
            .nth(1)?
            .parse::<u16>()
            .ok()?;
        Some(status_code == 200 && body.contains("ns8-swm-center"))
    };

    tokio::time::timeout(timeout, probe)
        .await
        .ok()
        .flatten()
        .unwrap_or(false)
}

pub async fn notify_existing_open_window(url: &str, timeout: Duration) -> bool {
    let Some(addr) = parse_session_loopback_addr(url) else {
        return false;
    };
    let post_call = async move {
        let mut stream = tokio::net::TcpStream::connect(addr).await.ok()?;
        let req = format!(
            "POST /api/open-window HTTP/1.1\r\nHost: {addr}\r\nContent-Type: application/json\r\nContent-Length: 2\r\nConnection: close\r\n\r\n{{}}"
        );
        stream.write_all(req.as_bytes()).await.ok()?;
        let mut buf = Vec::with_capacity(512);
        stream.read_to_end(&mut buf).await.ok()?;
        let raw = String::from_utf8_lossy(&buf);
        let first_line = raw.lines().next()?;
        let status_code = first_line.split_whitespace().nth(1)?.parse::<u16>().ok()?;
        Some(status_code == 200)
    };

    tokio::time::timeout(timeout, post_call)
        .await
        .ok()
        .flatten()
        .unwrap_or(false)
}

/// Checks `center-session.json` with a 300ms timeout. If a live `swm-center` instance responds
/// to `GET /api/ping` with `200 OK`, optionally triggers `POST /api/open-window` (when `!tray_only`)
/// and returns `true` so the caller can exit cleanly with `Ok(())` without creating a second tray icon.
pub async fn try_handoff_to_existing_instance(
    session_path: &Path,
    tray_only: bool,
    no_open: bool,
) -> bool {
    let Ok(raw) = fs::read_to_string(session_path) else {
        return false;
    };
    let Ok(info) = serde_json::from_str::<CenterSessionInfo>(&raw) else {
        return false;
    };
    let timeout = Duration::from_millis(300);
    if !probe_existing_center(&info.url, timeout).await {
        return false;
    }

    if !tray_only {
        if !notify_existing_open_window(&info.url, timeout).await && !no_open {
            crate::tray::launch_desktop_window(&info.url);
        }
    }
    true
}

pub fn write_session_file(session_path: &Path, url: &str) -> anyhow::Result<()> {
    if let Some(parent) = session_path.parent() {
        fs::create_dir_all(parent)?;
    }
    let payload = serde_json::to_vec(&CenterSessionInfo {
        url: url.to_string(),
    })?;
    fs::write(session_path, payload)?;
    Ok(())
}

pub fn cleanup_session_file_if_owned(session_path: &Path, url: &str) {
    if let Ok(raw) = fs::read_to_string(session_path) {
        if let Ok(info) = serde_json::from_str::<CenterSessionInfo>(&raw) {
            if info.url == url {
                let _ = fs::remove_file(session_path);
            }
        }
    }
}

pub fn embedded_asset_count() -> usize {
    EMBEDDED_UI_ASSETS.len()
}

pub fn lookup_embedded_asset(rel_path: &str) -> Option<&'static [u8]> {
    EMBEDDED_UI_ASSETS
        .binary_search_by_key(&rel_path, |(k, _)| *k)
        .ok()
        .map(|idx| EMBEDDED_UI_ASSETS[idx].1)
}

/// Discovers an optional live-dev UI directory override (`SWM_CENTER_UI_DIR`).
/// When `None` is returned, `swm-center` serves its compile-time embedded `ui/dist` bundle,
/// making `swm-center(.exe)` a true single-file standalone desktop binary.
pub fn discover_ui_dist_dir() -> Option<PathBuf> {
    if let Ok(env_dir) = std::env::var("SWM_CENTER_UI_DIR") {
        let trimmed = env_dir.trim();
        if !trimmed.is_empty() {
            let p = PathBuf::from(trimmed);
            if p.join("index.html").is_file() {
                return Some(p);
            }
        }
    }

    // Only consult disk fallback paths when compiled without embedded UI assets
    if EMBEDDED_UI_ASSETS.is_empty() {
        if let Ok(exe) = std::env::current_exe() {
            if let Some(dir) = exe.parent() {
                for candidate in [
                    dir.join("ui").join("dist"),
                    dir.join("swm-center-ui"),
                    dir.join("..").join("share").join("ns8-swm").join("center-ui"),
                ] {
                    if candidate.join("index.html").is_file() {
                        return Some(candidate);
                    }
                }
            }
        }
    }

    None
}

fn ipc_to_http(res: anyhow::Result<IpcResponse>) -> (StatusCode, Json<IpcResponse>) {
    match res {
        Ok(resp) => {
            let code = if resp.ok {
                StatusCode::OK
            } else {
                match resp.error_code {
                    Some(IpcErrorCode::PermissionDenied) => StatusCode::FORBIDDEN,
                    Some(IpcErrorCode::InvalidRequest) => StatusCode::BAD_REQUEST,
                    Some(IpcErrorCode::NotFound) => StatusCode::NOT_FOUND,
                    Some(IpcErrorCode::DeadlineExceeded) => StatusCode::CONFLICT,
                    _ => StatusCode::BAD_GATEWAY,
                }
            };
            (code, Json(resp))
        }
        Err(err) => (
            StatusCode::SERVICE_UNAVAILABLE,
            Json(IpcResponse::error(
                IpcErrorCode::AgentOffline,
                format!("swm-agent IPC unavailable: {err}"),
            )),
        ),
    }
}

async fn api_catalog(State(st): State<CenterBridgeState>) -> impl IntoResponse {
    ipc_to_http(send_ipc_request(&st.ipc_addr, &IpcRequest::ListCatalog).await)
}

async fn api_status(State(st): State<CenterBridgeState>) -> impl IntoResponse {
    ipc_to_http(send_ipc_request(&st.ipc_addr, &IpcRequest::GetStatus).await)
}

async fn api_trigger(
    State(st): State<CenterBridgeState>,
    Json(body): Json<TriggerPayload>,
) -> impl IntoResponse {
    ipc_to_http(
        send_ipc_request(
            &st.ipc_addr,
            &IpcRequest::TriggerPackage {
                package_id: body.package_id,
                action: body.action,
            },
        )
        .await,
    )
}

async fn api_trigger_updates(State(st): State<CenterBridgeState>) -> impl IntoResponse {
    ipc_to_http(send_ipc_request(&st.ipc_addr, &IpcRequest::TriggerPendingUpdates).await)
}

async fn api_snooze_reboot(
    State(st): State<CenterBridgeState>,
    Json(body): Json<SnoozePayload>,
) -> impl IntoResponse {
    ipc_to_http(
        send_ipc_request(
            &st.ipc_addr,
            &IpcRequest::SnoozeReboot {
                minutes: body.minutes,
            },
        )
        .await,
    )
}

async fn api_confirm_reboot(State(st): State<CenterBridgeState>) -> impl IntoResponse {
    ipc_to_http(send_ipc_request(&st.ipc_addr, &IpcRequest::ConfirmRebootNow).await)
}

async fn api_ping() -> impl IntoResponse {
    (
        StatusCode::OK,
        Json(serde_json::json!({
            "ok": true,
            "service": "ns8-swm-center"
        })),
    )
}

async fn api_open_window(State(st): State<CenterBridgeState>) -> impl IntoResponse {
    st.open_window_count.fetch_add(1, Ordering::SeqCst);
    if !st.no_open && !st.local_url.is_empty() {
        crate::tray::launch_desktop_window(&st.local_url);
    }
    (StatusCode::OK, Json(serde_json::json!({ "ok": true })))
}

async fn api_sync(State(st): State<CenterBridgeState>) -> impl IntoResponse {
    ipc_to_http(send_ipc_request(&st.ipc_addr, &IpcRequest::SyncNow).await)
}

async fn api_dismiss_message(
    State(st): State<CenterBridgeState>,
    Json(body): Json<DismissMessagePayload>,
) -> impl IntoResponse {
    ipc_to_http(
        send_ipc_request(
            &st.ipc_addr,
            &IpcRequest::DismissMessage {
                message_id: body.message_id,
            },
        )
        .await,
    )
}

pub fn guess_mime(path: &Path) -> &'static str {
    match path
        .extension()
        .and_then(|e| e.to_str())
        .unwrap_or("")
        .to_ascii_lowercase()
        .as_str()
    {
        "html" => "text/html; charset=utf-8",
        "js" | "mjs" => "application/javascript; charset=utf-8",
        "css" => "text/css; charset=utf-8",
        "svg" => "image/svg+xml",
        "png" => "image/png",
        "ico" => "image/x-icon",
        "webp" => "image/webp",
        "woff2" => "font/woff2",
        "woff" => "font/woff",
        "json" => "application/json",
        _ => "application/octet-stream",
    }
}

fn is_static_asset_request(clean_path: &str) -> bool {
    if clean_path.starts_with("assets/") {
        return true;
    }
    matches!(
        Path::new(clean_path)
            .extension()
            .and_then(|e| e.to_str())
            .unwrap_or("")
            .to_ascii_lowercase()
            .as_str(),
        "js" | "mjs" | "css" | "svg" | "png" | "ico" | "webp" | "woff" | "woff2" | "map" | "json"
    )
}

const FALLBACK_INDEX_HTML: &str = r#"<!DOCTYPE html>
<html lang="de">
<head><meta charset="utf-8"><title>NS8 Software Center</title></head>
<body><h1>NS8 Software Center</h1><p>UI-Bundle wird geladen…</p></body>
</html>"#;

fn serve_dist_file(dist_opt: &Option<PathBuf>, rel_path: &str) -> Response {
    let clean = rel_path.trim_start_matches('/');
    if clean.contains("..") || clean.contains('\\') {
        return Response::builder()
            .status(StatusCode::BAD_REQUEST)
            .header(header::CONTENT_TYPE, "text/plain; charset=utf-8")
            .body(Body::from("Invalid asset path"))
            .unwrap();
    }

    let target_rel = if clean.is_empty() { "index.html" } else { clean };

    // 1. Highest priority: explicit SWM_CENTER_UI_DIR disk override (live dev / tests)
    if let Some(dist_dir) = dist_opt {
        let candidate = dist_dir.join(target_rel);
        if candidate.is_file() {
            if let Ok(bytes) = fs::read(&candidate) {
                return Response::builder()
                    .status(StatusCode::OK)
                    .header(header::CONTENT_TYPE, guess_mime(&candidate))
                    .body(Body::from(bytes))
                    .unwrap();
            }
        }
        if is_static_asset_request(target_rel) {
            return Response::builder()
                .status(StatusCode::NOT_FOUND)
                .header(header::CONTENT_TYPE, "text/plain; charset=utf-8")
                .body(Body::from("Asset not found"))
                .unwrap();
        }
        let spa_index = dist_dir.join("index.html");
        if let Ok(bytes) = fs::read(&spa_index) {
            return Response::builder()
                .status(StatusCode::OK)
                .header(header::CONTENT_TYPE, "text/html; charset=utf-8")
                .body(Body::from(bytes))
                .unwrap();
        }
    }

    // 2. Single-file compile-time embedded Vue 3 SPA bundle (ui/dist)
    if let Some(bytes) = lookup_embedded_asset(target_rel) {
        return Response::builder()
            .status(StatusCode::OK)
            .header(header::CONTENT_TYPE, guess_mime(Path::new(target_rel)))
            .body(Body::from(bytes))
            .unwrap();
    }

    if is_static_asset_request(target_rel) && !EMBEDDED_UI_ASSETS.is_empty() {
        return Response::builder()
            .status(StatusCode::NOT_FOUND)
            .header(header::CONTENT_TYPE, "text/plain; charset=utf-8")
            .body(Body::from("Embedded asset not found"))
            .unwrap();
    }

    if let Some(index_bytes) = lookup_embedded_asset("index.html") {
        return Response::builder()
            .status(StatusCode::OK)
            .header(header::CONTENT_TYPE, "text/html; charset=utf-8")
            .body(Body::from(index_bytes))
            .unwrap();
    }

    // 3. Bare fallback only if ui/dist had not been built prior to compilation
    Response::builder()
        .status(StatusCode::OK)
        .header(header::CONTENT_TYPE, "text/html; charset=utf-8")
        .body(Body::from(FALLBACK_INDEX_HTML))
        .unwrap()
}

async fn serve_root(State(st): State<CenterBridgeState>) -> Response {
    serve_dist_file(&st.ui_dist_dir, "index.html")
}

async fn serve_subpath(
    State(st): State<CenterBridgeState>,
    AxumPath(path): AxumPath<String>,
) -> Response {
    serve_dist_file(&st.ui_dist_dir, &path)
}

pub fn build_center_router(state: CenterBridgeState) -> Router {
    Router::new()
        .route("/api/ping", get(api_ping))
        .route("/api/open-window", post(api_open_window))
        .route("/api/sync", post(api_sync))
        .route("/api/messages/dismiss", post(api_dismiss_message))
        .route("/api/center/catalog", get(api_catalog))
        .route("/api/center/status", get(api_status))
        .route("/api/center/trigger", post(api_trigger))
        .route("/api/center/trigger-updates", post(api_trigger_updates))
        .route("/api/center/snooze-reboot", post(api_snooze_reboot))
        .route("/api/center/confirm-reboot", post(api_confirm_reboot))
        .route("/api/center/sync", post(api_sync))
        .route("/api/center/messages/dismiss", post(api_dismiss_message))
        .route("/", get(serve_root))
        .route("/*path", get(serve_subpath))
        .with_state(state)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::net::SocketAddr;
    use tokio::io::{AsyncBufReadExt, AsyncReadExt, AsyncWriteExt, BufReader};

    async fn raw_http_get(addr: SocketAddr, path: &str) -> (u16, String, String) {
        let mut stream = tokio::net::TcpStream::connect(addr).await.unwrap();
        let req = format!(
            "GET {path} HTTP/1.1\r\nHost: {addr}\r\nConnection: close\r\n\r\n"
        );
        stream.write_all(req.as_bytes()).await.unwrap();
        let mut buf = Vec::new();
        stream.read_to_end(&mut buf).await.unwrap();
        let raw = String::from_utf8_lossy(&buf).into_owned();
        let (head, body) = raw.split_once("\r\n\r\n").unwrap_or((&raw, ""));
        let first_line = head.lines().next().unwrap_or("");
        let status = first_line
            .split_whitespace()
            .nth(1)
            .and_then(|s| s.parse::<u16>().ok())
            .unwrap_or(0);
        (status, head.to_ascii_lowercase(), body.to_string())
    }

    #[tokio::test]
    async fn test_center_bridge_forwards_unprivileged_requests_over_ipc() {
        let ipc_listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let ipc_addr = ipc_listener.local_addr().unwrap().to_string();

        tokio::spawn(async move {
            while let Ok((stream, _)) = ipc_listener.accept().await {
                let (r, mut w) = tokio::io::split(stream);
                let mut br = BufReader::new(r);
                let mut line = String::new();
                if br.read_line(&mut line).await.unwrap_or(0) > 0 {
                    let req: IpcRequest = serde_json::from_str(line.trim()).unwrap();
                    let resp = match req {
                        IpcRequest::ListCatalog => IpcResponse::success(
                            swm_common::IpcResponseData::Catalog { items: vec![] },
                        ),
                        IpcRequest::TriggerPackage { package_id, .. }
                            if package_id == "unauthorized" =>
                        {
                            IpcResponse::error(IpcErrorCode::PermissionDenied, "Not allowed")
                        }
                        _ => IpcResponse::error(IpcErrorCode::InvalidRequest, "other"),
                    };
                    let mut raw = serde_json::to_vec(&resp).unwrap();
                    raw.push(b'\n');
                    let _ = w.write_all(&raw).await;
                }
            }
        });

        let bridge_app = build_center_router(CenterBridgeState {
            ipc_addr: ipc_addr.clone(),
            ui_dist_dir: None,
            ..Default::default()
        });
        let http_listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let http_addr = http_listener.local_addr().unwrap();
        tokio::spawn(async move {
            axum::serve(http_listener, bridge_app).await.unwrap();
        });

        let cat = send_ipc_request(&ipc_addr, &IpcRequest::ListCatalog)
            .await
            .unwrap();
        assert!(cat.ok);

        let denied = send_ipc_request(
            &ipc_addr,
            &IpcRequest::TriggerPackage {
                package_id: "unauthorized".to_string(),
                action: PackageAction::Install,
            },
        )
        .await
        .unwrap();
        assert!(!denied.ok);
        assert_eq!(denied.error_code, Some(IpcErrorCode::PermissionDenied));
        let _ = http_addr;
    }

    #[tokio::test]
    async fn test_embedded_ui_bundle_serves_vue_spa_and_assets_without_disk_dir() {
        assert!(
            embedded_asset_count() >= 3,
            "Expected built ui/dist assets (index.html, JS, CSS) embedded in swm-center"
        );

        let bridge_app = build_center_router(CenterBridgeState {
            ipc_addr: "127.0.0.1:9".to_string(),
            ui_dist_dir: None,
            ..Default::default()
        });
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let addr = listener.local_addr().unwrap();
        tokio::spawn(async move {
            axum::serve(listener, bridge_app).await.unwrap();
        });

        // 1. Root "/" must serve the real embedded Vue 3 SPA, NOT FALLBACK_INDEX_HTML
        let (status, headers, body) = raw_http_get(addr, "/").await;
        assert_eq!(status, 200);
        assert!(headers.contains("content-type: text/html; charset=utf-8"));
        assert!(
            body.contains("<div id=\"app\"></div>"),
            "Expected Vue mount container <div id=\"app\"></div>, got: {body}"
        );
        assert!(
            !body.contains("UI-Bundle wird geladen"),
            "Must serve embedded SPA, not placeholder stub"
        );

        // 2. Verify embedded JS & CSS assets referenced in index.html are served with proper MIME types
        let js_asset = EMBEDDED_UI_ASSETS
            .iter()
            .find(|(k, _)| k.starts_with("assets/") && k.ends_with(".js"))
            .map(|(k, _)| *k)
            .expect("Expected embedded .js bundle in assets/");
        let (js_status, js_headers, js_body) =
            raw_http_get(addr, &format!("/{js_asset}")).await;
        assert_eq!(js_status, 200);
        assert!(js_headers.contains("content-type: application/javascript; charset=utf-8"));
        assert!(js_body.len() > 100);

        let css_asset = EMBEDDED_UI_ASSETS
            .iter()
            .find(|(k, _)| k.starts_with("assets/") && k.ends_with(".css"))
            .map(|(k, _)| *k)
            .expect("Expected embedded .css bundle in assets/");
        let (css_status, css_headers, css_body) =
            raw_http_get(addr, &format!("/{css_asset}")).await;
        assert_eq!(css_status, 200);
        assert!(css_headers.contains("content-type: text/css; charset=utf-8"));
        assert!(css_body.len() > 50);

        // Verify embedded SVG brand logo is served with image/svg+xml MIME header
        let (svg_status, svg_headers, svg_body) = raw_http_get(addr, "/logo.svg").await;
        assert_eq!(svg_status, 200);
        assert!(svg_headers.contains("content-type: image/svg+xml"));
        assert!(svg_body.contains("<svg"));

        // 3. SPA client sub-route falls back to index.html, while missing /assets/* returns 404
        let (spa_status, _, spa_body) = raw_http_get(addr, "/updates").await;
        assert_eq!(spa_status, 200);
        assert!(spa_body.contains("<div id=\"app\"></div>"));

        let (missing_status, _, _) = raw_http_get(addr, "/assets/does-not-exist.js").await;
        assert_eq!(missing_status, 404);
    }

    #[tokio::test]
    async fn test_ui_dist_dir_override_takes_priority_over_embedded() {
        let tmp = tempfile::tempdir().unwrap();
        fs::write(
            tmp.path().join("index.html"),
            "<!DOCTYPE html><html><body><div id=\"app\">CUSTOM_OVERRIDE_MARKER</div></body></html>",
        )
        .unwrap();

        let bridge_app = build_center_router(CenterBridgeState {
            ipc_addr: "127.0.0.1:9".to_string(),
            ui_dist_dir: Some(tmp.path().to_path_buf()),
            ..Default::default()
        });
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let addr = listener.local_addr().unwrap();
        tokio::spawn(async move {
            axum::serve(listener, bridge_app).await.unwrap();
        });

        let (status, _, body) = raw_http_get(addr, "/").await;
        assert_eq!(status, 200);
        assert!(body.contains("CUSTOM_OVERRIDE_MARKER"));
    }

    #[test]
    fn test_client_packaging_autostart_and_pure_lf_invariants() {
        let pkg_dir = Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("..")
            .join("..")
            .join("packaging");

        let ps1 = fs::read_to_string(pkg_dir.join("install.ps1")).unwrap();
        assert!(ps1.contains("$Headless -eq 0"));
        assert!(ps1.contains(r"HKLM:\Software\Microsoft\Windows\CurrentVersion\Run"));
        assert!(ps1.contains("NS8SoftwareCenter"));
        assert!(ps1.contains(r#""C:\Program Files\NS8-SoftwareManager\swm-center.exe" gui --tray-only"#));

        let wxs = fs::read_to_string(pkg_dir.join("ns8-swm.wxs")).unwrap();
        assert!(wxs.contains(r#"Key="Software\Microsoft\Windows\CurrentVersion\Run""#));
        assert!(wxs.contains(r#"Name="NS8SoftwareCenter""#));
        assert!(wxs.contains(r#"&quot;C:\Program Files\NS8-SoftwareManager\swm-center.exe&quot; gui --tray-only"#));

        for lf_file in [
            "install.sh",
            "swm-agent.service",
            "swm-center.desktop",
            "nfpm.yaml",
        ] {
            let raw = fs::read(pkg_dir.join(lf_file)).unwrap();
            assert!(
                !raw.windows(2).any(|w| w == b"\r\n"),
                "CRLF detected in packaging/{lf_file}"
            );
        }

        let sh = fs::read_to_string(pkg_dir.join("install.sh")).unwrap();
        assert!(sh.contains("/etc/xdg/autostart/swm-center.desktop"));
        assert!(sh.contains("Exec=/usr/local/bin/swm-center gui --tray-only"));

        let desktop = fs::read_to_string(pkg_dir.join("swm-center.desktop")).unwrap();
        assert!(desktop.contains("Exec=/usr/local/bin/swm-center gui --tray-only"));

        let nfpm = fs::read_to_string(pkg_dir.join("nfpm.yaml")).unwrap();
        assert!(nfpm.contains("dst: \"/etc/xdg/autostart/swm-center.desktop\""));
        assert!(nfpm.contains("dst: \"/usr/local/bin/swm-center\""));
    }

    #[tokio::test]
    async fn test_single_instance_handoff_ping_open_window_and_stale_recovery() {
        let tmp = tempfile::tempdir().unwrap();
        let session_file = tmp.path().join("center-session.json");

        // 1. Stale session pointing to closed loopback port -> returns false so new instance binds
        write_session_file(&session_file, "http://127.0.0.1:9").unwrap();
        assert!(!try_handoff_to_existing_instance(&session_file, false, true).await);

        // 2. Start live bridge instance and record its URL in center-session.json
        let open_counter = Arc::new(AtomicUsize::new(0));
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let addr = listener.local_addr().unwrap();
        let live_url = format!("http://{addr}");
        let bridge_app = build_center_router(CenterBridgeState {
            ipc_addr: "127.0.0.1:9".to_string(),
            ui_dist_dir: None,
            local_url: live_url.clone(),
            no_open: true,
            open_window_count: Arc::clone(&open_counter),
        });
        tokio::spawn(async move {
            axum::serve(listener, bridge_app).await.unwrap();
        });
        write_session_file(&session_file, &live_url).unwrap();

        // Verify GET /api/ping directly
        let (ping_status, _, ping_body) = raw_http_get(addr, "/api/ping").await;
        assert_eq!(ping_status, 200);
        assert!(ping_body.contains("ns8-swm-center"));

        // 3. Secondary call with tray_only=true detects running instance WITHOUT opening window
        assert!(try_handoff_to_existing_instance(&session_file, true, true).await);
        assert_eq!(open_counter.load(Ordering::SeqCst), 0);

        // 4. Secondary interactive call (tray_only=false) detects running instance AND calls POST /api/open-window
        assert!(try_handoff_to_existing_instance(&session_file, false, true).await);
        assert_eq!(open_counter.load(Ordering::SeqCst), 1);
    }
}
