use anyhow::{Context, Result};
use futures_util::{SinkExt, StreamExt};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::time::Duration;
use swm_common::{WsAgentFrame, WsServerFrame, AGENT_VERSION};
use tokio::sync::{mpsc, watch};
use tokio_tungstenite::connect_async;
use tokio_tungstenite::tungstenite::Message;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum WsServerSignal {
    JobAvailable(Option<String>),
    CatalogUpdated,
    Revoked,
}

pub fn build_agent_ws_url(server_url: &str, device_token: &str) -> Result<String> {
    let base = server_url.trim().trim_end_matches('/');
    let ws_base = if let Some(rest) = base.strip_prefix("https://") {
        format!("wss://{rest}")
    } else if let Some(rest) = base.strip_prefix("http://") {
        format!("ws://{rest}")
    } else {
        format!("wss://{base}")
    };

    let mut parsed = url::Url::parse(&format!("{ws_base}/api/v1/agent/ws"))
        .context("Invalid server URL for WebSocket endpoint")?;
    parsed.query_pairs_mut().append_pair("token", device_token);
    Ok(parsed.to_string())
}

pub fn compute_backoff_secs(attempt: u32) -> u64 {
    let shift = attempt.min(6);
    (1u64 << shift).min(60)
}

/// Persistent outbound WebSocket task (`WS /api/v1/agent/ws?token=<device_token>`)
/// with exponential reconnect backoff, periodic heartbeats, live `log_chunk` forwarding,
/// and instant `job_available` / `catalog_updated` / `revoked` push handling.
pub async fn run_ws_supervisor(
    server_url: String,
    device_token: String,
    ws_connected: Arc<AtomicBool>,
    reboot_required_flag: Arc<AtomicBool>,
    mut outgoing_rx: mpsc::Receiver<WsAgentFrame>,
    signal_tx: mpsc::Sender<WsServerSignal>,
    mut shutdown_rx: watch::Receiver<bool>,
) {
    let Ok(ws_url) = build_agent_ws_url(&server_url, &device_token) else {
        tracing::error!("Invalid WebSocket URL derived from {server_url}");
        return;
    };

    let mut failed_attempts = 0u32;

    loop {
        if *shutdown_rx.borrow() {
            break;
        }

        match connect_async(&ws_url).await {
            Ok((ws_stream, _resp)) => {
                failed_attempts = 0;
                ws_connected.store(true, Ordering::SeqCst);
                tracing::info!("Connected to server WebSocket channel");

                let (mut sink, mut stream) = ws_stream.split();

                // Send initial heartbeat immediately upon connecting
                let init_hb = WsAgentFrame::Heartbeat {
                    agent_version: AGENT_VERSION.to_string(),
                    reboot_required: reboot_required_flag.load(Ordering::SeqCst),
                };
                if let Ok(txt) = serde_json::to_string(&init_hb) {
                    let _ = sink.send(Message::Text(txt)).await;
                }

                // Trigger an immediate sync on reconnect to catch anything missed while offline
                let _ = signal_tx.send(WsServerSignal::JobAvailable(None)).await;

                let mut heartbeat_interval = tokio::time::interval(Duration::from_secs(20));
                heartbeat_interval.tick().await; // consume immediate first tick

                let mut revoked = false;

                loop {
                    tokio::select! {
                        _ = shutdown_rx.changed() => {
                            if *shutdown_rx.borrow() {
                                let _ = sink.close().await;
                                ws_connected.store(false, Ordering::SeqCst);
                                return;
                            }
                        }
                        _ = heartbeat_interval.tick() => {
                            let hb = WsAgentFrame::Heartbeat {
                                agent_version: AGENT_VERSION.to_string(),
                                reboot_required: reboot_required_flag.load(Ordering::SeqCst),
                            };
                            if let Ok(txt) = serde_json::to_string(&hb) {
                                if sink.send(Message::Text(txt)).await.is_err() {
                                    break;
                                }
                            }
                        }
                        Some(frame) = outgoing_rx.recv() => {
                            if let Ok(txt) = serde_json::to_string(&frame) {
                                if sink.send(Message::Text(txt)).await.is_err() {
                                    break;
                                }
                            }
                        }
                        incoming = stream.next() => {
                            match incoming {
                                Some(Ok(Message::Text(txt))) => {
                                    if let Ok(srv) = serde_json::from_str::<WsServerFrame>(&txt) {
                                        match srv.frame_type.as_str() {
                                            "job_available" => {
                                                let _ = signal_tx.send(WsServerSignal::JobAvailable(srv.job_id)).await;
                                            }
                                            "catalog_updated" => {
                                                let _ = signal_tx.send(WsServerSignal::CatalogUpdated).await;
                                            }
                                            "revoked" => {
                                                let _ = signal_tx.send(WsServerSignal::Revoked).await;
                                                revoked = true;
                                                break;
                                            }
                                            _ => {}
                                        }
                                    }
                                }
                                Some(Ok(Message::Ping(payload))) => {
                                    let _ = sink.send(Message::Pong(payload)).await;
                                }
                                Some(Ok(Message::Close(close_frame))) => {
                                    if let Some(cf) = close_frame {
                                        if u16::from(cf.code) == 1008 {
                                            let _ = signal_tx.send(WsServerSignal::Revoked).await;
                                            revoked = true;
                                        }
                                    }
                                    break;
                                }
                                None | Some(Err(_)) => break,
                                _ => {}
                            }
                        }
                    }
                }

                ws_connected.store(false, Ordering::SeqCst);
                if revoked {
                    tracing::warn!("Device revoked by server; stopping WebSocket supervisor");
                    return;
                }
            }
            Err(err) => {
                ws_connected.store(false, Ordering::SeqCst);
                failed_attempts = failed_attempts.saturating_add(1);
                tracing::debug!(
                    "WebSocket connection attempt {} failed: {}",
                    failed_attempts,
                    err
                );
            }
        }

        let backoff = Duration::from_secs(compute_backoff_secs(failed_attempts));
        tokio::select! {
            _ = tokio::time::sleep(backoff) => {}
            _ = shutdown_rx.changed() => {
                if *shutdown_rx.borrow() {
                    break;
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use axum::extract::ws::{Message as AxumWsMsg, WebSocket, WebSocketUpgrade};
    use axum::response::IntoResponse;
    use axum::routing::get;
    use axum::Router;

    #[test]
    fn test_ws_url_conversion_and_backoff_curve() {
        assert_eq!(
            build_agent_ws_url("https://swm.company.de/", "tok_123").unwrap(),
            "wss://swm.company.de/api/v1/agent/ws?token=tok_123"
        );
        assert_eq!(
            build_agent_ws_url("http://127.0.0.1:8080", "tok_abc").unwrap(),
            "ws://127.0.0.1:8080/api/v1/agent/ws?token=tok_abc"
        );

        assert_eq!(compute_backoff_secs(0), 1);
        assert_eq!(compute_backoff_secs(1), 2);
        assert_eq!(compute_backoff_secs(3), 8);
        assert_eq!(compute_backoff_secs(10), 60);
    }

    async fn mock_ws_handler(ws: WebSocketUpgrade) -> impl IntoResponse {
        ws.on_upgrade(|mut socket: WebSocket| async move {
            // Expect initial heartbeat from agent, then push job_available & catalog_updated
            if let Some(Ok(AxumWsMsg::Text(first))) = socket.recv().await {
                assert!(first.contains("\"heartbeat\""));
                let _ = socket
                    .send(AxumWsMsg::Text(
                        r#"{"type":"job_available","job_id":"job-99"}"#.into(),
                    ))
                    .await;
                let _ = socket
                    .send(AxumWsMsg::Text(r#"{"type":"catalog_updated"}"#.into()))
                    .await;
            }
            tokio::time::sleep(Duration::from_millis(200)).await;
        })
    }

    #[tokio::test]
    async fn test_ws_supervisor_heartbeat_and_push_signals() {
        let app = Router::new().route("/api/v1/agent/ws", get(mock_ws_handler));
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let addr = listener.local_addr().unwrap();
        tokio::spawn(async move {
            axum::serve(listener, app).await.unwrap();
        });

        let ws_connected = Arc::new(AtomicBool::new(false));
        let reboot_flag = Arc::new(AtomicBool::new(false));
        let (_out_tx, out_rx) = mpsc::channel(8);
        let (sig_tx, mut sig_rx) = mpsc::channel(8);
        let (shut_tx, shut_rx) = watch::channel(false);

        let handle = tokio::spawn(run_ws_supervisor(
            format!("http://{addr}"),
            "dev_token_1".to_string(),
            ws_connected.clone(),
            reboot_flag,
            out_rx,
            sig_tx,
            shut_rx,
        ));

        // Initial reconnect signal -> JobAvailable(None)
        let s1 = tokio::time::timeout(Duration::from_secs(3), sig_rx.recv())
            .await
            .unwrap()
            .unwrap();
        assert_eq!(s1, WsServerSignal::JobAvailable(None));

        // Server push -> JobAvailable(Some("job-99"))
        let s2 = tokio::time::timeout(Duration::from_secs(3), sig_rx.recv())
            .await
            .unwrap()
            .unwrap();
        assert_eq!(s2, WsServerSignal::JobAvailable(Some("job-99".to_string())));

        // Server push -> CatalogUpdated
        let s3 = tokio::time::timeout(Duration::from_secs(3), sig_rx.recv())
            .await
            .unwrap()
            .unwrap();
        assert_eq!(s3, WsServerSignal::CatalogUpdated);

        let _ = shut_tx.send(true);
        let _ = handle.await;
    }
}
