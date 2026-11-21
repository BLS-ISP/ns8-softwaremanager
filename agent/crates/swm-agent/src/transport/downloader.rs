use anyhow::{bail, Context, Result};
use futures_util::StreamExt;
use reqwest::header::RANGE;
use reqwest::{Client, StatusCode};
use std::fs::{self, OpenOptions};
use std::io::Write;
use std::path::{Path, PathBuf};
use std::time::Duration;
use swm_common::verify_file_sha256;

#[derive(Clone)]
pub struct ResumableDownloader {
    http: Client,
    max_retries: u32,
}

impl ResumableDownloader {
    pub fn new(http: Client) -> Self {
        Self {
            http,
            max_retries: 4,
        }
    }

    pub fn with_max_retries(mut self, max_retries: u32) -> Self {
        self.max_retries = max_retries.max(1);
        self
    }

    /// Downloads `url` into `dest_path` using HTTP `Range: bytes=<offset>-` resumption
    /// whenever an interrupted `.part` file exists or a connection drops mid-transfer,
    /// and strictly validates the full `SHA-256` digest BEFORE moving `.part` into place!
    pub async fn download_verified(
        &self,
        url: &str,
        bearer_token: Option<&str>,
        dest_path: &Path,
        expected_sha256: &str,
    ) -> Result<PathBuf> {
        let clean_sha = expected_sha256.trim().to_ascii_lowercase();
        if clean_sha.is_empty() {
            bail!("Security violation: custom/binary download requires a non-empty SHA-256 hash");
        }

        if let Some(parent) = dest_path.parent() {
            fs::create_dir_all(parent)?;
        }

        // Fast-path: if dest_path already exists with matching SHA-256, reuse verified artifact
        if dest_path.is_file() && verify_file_sha256(dest_path, &clean_sha).is_ok() {
            return Ok(dest_path.to_path_buf());
        }

        let part_path = dest_path.with_extension(format!(
            "{}.part",
            dest_path
                .extension()
                .and_then(|e| e.to_str())
                .unwrap_or("bin")
        ));

        let mut attempt = 0u32;
        loop {
            attempt += 1;
            let existing_offset = fs::metadata(&part_path).map(|m| m.len()).unwrap_or(0);

            let mut req = self.http.get(url);
            if let Some(tok) = bearer_token.filter(|t| !t.is_empty()) {
                req = req.bearer_auth(tok);
            }
            if existing_offset > 0 {
                req = req.header(RANGE, format!("bytes={existing_offset}-"));
            }

            let step_result: Result<()> = async {
                let resp = req
                    .send()
                    .await
                    .with_context(|| format!("Failed connecting to {url}"))?;
                let status = resp.status();

                if status == StatusCode::RANGE_NOT_SATISFIABLE {
                    // Range already complete or stale .part size -> check SHA-256 first, otherwise reset .part
                    if verify_file_sha256(&part_path, &clean_sha).is_ok() {
                        return Ok(());
                    }
                    let _ = fs::remove_file(&part_path);
                    bail!("Range 416 with mismatched SHA-256, resetting partial file");
                }

                if !status.is_success() {
                    let body = resp.text().await.unwrap_or_default();
                    bail!("Package download failed ({status}): {body}");
                }

                if let Some(hdr_sha) = resp
                    .headers()
                    .get("X-Package-SHA256")
                    .and_then(|v| v.to_str().ok())
                {
                    let hdr_clean = hdr_sha.trim().to_ascii_lowercase();
                    if !hdr_clean.is_empty() && hdr_clean != clean_sha {
                        let _ = fs::remove_file(&part_path);
                        bail!(
                            "Security violation: X-Package-SHA256 header ({hdr_clean}) != signed SHA-256 ({clean_sha})"
                        );
                    }
                }

                let mut file = if status == StatusCode::PARTIAL_CONTENT && existing_offset > 0 {
                    OpenOptions::new()
                        .create(true)
                        .append(true)
                        .open(&part_path)?
                } else {
                    OpenOptions::new()
                        .create(true)
                        .write(true)
                        .truncate(true)
                        .open(&part_path)?
                };

                let mut stream = resp.bytes_stream();
                while let Some(chunk_res) = stream.next().await {
                    let chunk = chunk_res.context("Interrupted while streaming package bytes")?;
                    file.write_all(&chunk)?;
                }
                file.flush()?;
                Ok(())
            }
            .await;

            match step_result {
                Ok(()) => break,
                Err(err) if attempt < self.max_retries => {
                    tracing::warn!(
                        "Download attempt {attempt}/{} interrupted ({err}), resuming via HTTP Range...",
                        self.max_retries
                    );
                    tokio::time::sleep(Duration::from_millis(150 * u64::from(attempt))).await;
                }
                Err(err) => return Err(err),
            }
        }

        if let Err(crypto_err) = verify_file_sha256(&part_path, &clean_sha) {
            let _ = fs::remove_file(&part_path);
            bail!("Package integrity verification failed: {crypto_err}");
        }

        if dest_path.exists() {
            let _ = fs::remove_file(dest_path);
        }
        fs::rename(&part_path, dest_path).with_context(|| {
            format!(
                "Failed moving verified download {} -> {}",
                part_path.display(),
                dest_path.display()
            )
        })?;

        Ok(dest_path.to_path_buf())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use axum::body::Body;
    use axum::extract::State;
    use axum::http::{HeaderMap, StatusCode as AxumStatus};
    use axum::response::Response;
    use axum::routing::get;
    use axum::Router;
    use std::sync::atomic::{AtomicUsize, Ordering};
    use std::sync::Arc;
    use swm_common::compute_bytes_sha256;
    use tempfile::tempdir;

    #[derive(Clone)]
    struct TestRangeState {
        payload: Arc<Vec<u8>>,
        sha256: String,
        range_requests_seen: Arc<AtomicUsize>,
    }

    async fn range_handler(
        State(st): State<TestRangeState>,
        headers: HeaderMap,
    ) -> Response<Body> {
        let total = st.payload.len();
        if let Some(range_val) = headers.get("range").and_then(|v| v.to_str().ok()) {
            if let Some(rest) = range_val.strip_prefix("bytes=") {
                let start_str = rest.split('-').next().unwrap_or("0");
                if let Ok(start) = start_str.parse::<usize>() {
                    if start < total {
                        st.range_requests_seen.fetch_add(1, Ordering::SeqCst);
                        let slice = st.payload[start..].to_vec();
                        return Response::builder()
                            .status(AxumStatus::PARTIAL_CONTENT)
                            .header("Accept-Ranges", "bytes")
                            .header("X-Package-SHA256", &st.sha256)
                            .header(
                                "Content-Range",
                                format!("bytes {}-{}/{}", start, total - 1, total),
                            )
                            .body(Body::from(slice))
                            .unwrap();
                    }
                }
            }
        }

        Response::builder()
            .status(AxumStatus::OK)
            .header("Accept-Ranges", "bytes")
            .header("X-Package-SHA256", &st.sha256)
            .body(Body::from((*st.payload).clone()))
            .unwrap()
    }

    #[tokio::test]
    async fn test_downloader_resumes_partial_file_with_206_and_verifies_sha256() {
        let payload: Vec<u8> = (0..131_072).map(|i| (i % 251) as u8).collect();
        let sha = compute_bytes_sha256(&payload);
        let range_hits = Arc::new(AtomicUsize::new(0));

        let app = Router::new()
            .route("/api/v1/agent/packages/pkg-1/download", get(range_handler))
            .with_state(TestRangeState {
                payload: Arc::new(payload.clone()),
                sha256: sha.clone(),
                range_requests_seen: range_hits.clone(),
            });

        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let addr = listener.local_addr().unwrap();
        tokio::spawn(async move {
            axum::serve(listener, app).await.unwrap();
        });

        let dir = tempdir().unwrap();
        let dest = dir.path().join("installer.msi");
        let part = dir.path().join("installer.msi.part");

        // Pre-seed the first 50,000 bytes as an interrupted partial download
        fs::write(&part, &payload[..50_000]).unwrap();

        let downloader = ResumableDownloader::new(Client::new());
        let url = format!("http://{addr}/api/v1/agent/packages/pkg-1/download");
        let out_path = downloader
            .download_verified(&url, Some("dev-tok"), &dest, &sha)
            .await
            .unwrap();

        assert_eq!(out_path, dest);
        assert_eq!(range_hits.load(Ordering::SeqCst), 1);
        assert!(!part.exists());
        assert_eq!(fs::read(&dest).unwrap(), payload);

        // Tampered expected SHA-256 must fail deterministically!
        let dest_bad = dir.path().join("tampered.msi");
        let bad_res = downloader
            .download_verified(
                &url,
                Some("dev-tok"),
                &dest_bad,
                "0000000000000000000000000000000000000000000000000000000000000000",
            )
            .await;
        assert!(bad_res.is_err());
        assert!(!dest_bad.exists());
    }
}
