use crate::ipc::{IpcRequest, IpcResponse, MAX_IPC_FRAME_BYTES};
use anyhow::{bail, Context, Result};
use std::net::SocketAddr;
use std::time::Duration;
use tokio::io::{AsyncBufReadExt, AsyncRead, AsyncWrite, AsyncWriteExt, BufReader};

/// Validates and parses a loopback TCP address if `addr` is not a Named Pipe or Unix Socket.
/// Strictly rejects any non-loopback IP address so local IPC can never be exposed over the network.
pub fn parse_loopback_tcp_addr(addr: &str) -> Result<Option<SocketAddr>> {
    let trimmed = addr.trim();
    if trimmed.starts_with(r"\\.\pipe\") || trimmed.starts_with('/') || trimmed.starts_with("unix:")
    {
        return Ok(None);
    }
    let raw = trimmed.strip_prefix("tcp://").unwrap_or(trimmed);
    let normalized = if let Some(port) = raw.strip_prefix("localhost:") {
        format!("127.0.0.1:{port}")
    } else {
        raw.to_string()
    };
    let sock: SocketAddr = normalized
        .parse()
        .with_context(|| format!("Invalid IPC address '{addr}'"))?;
    if !sock.ip().is_loopback() {
        bail!(
            "Security violation: IPC TCP address '{}' must be loopback (127.0.0.1 or ::1)",
            sock
        );
    }
    Ok(Some(sock))
}

async fn exchange_line<S>(stream: S, req: &IpcRequest) -> Result<IpcResponse>
where
    S: AsyncRead + AsyncWrite + Unpin,
{
    let (reader, mut writer) = tokio::io::split(stream);
    let mut payload = serde_json::to_vec(req).context("Failed to serialize IPC request")?;
    if payload.len() > MAX_IPC_FRAME_BYTES {
        bail!("IPC request exceeds maximum frame size");
    }
    payload.push(b'\n');

    writer
        .write_all(&payload)
        .await
        .context("Failed writing to IPC stream")?;
    writer.flush().await.context("Failed flushing IPC stream")?;

    let mut buf_reader = BufReader::new(reader);
    let mut line = String::new();
    let read_res = tokio::time::timeout(Duration::from_secs(60), buf_reader.read_line(&mut line))
        .await
        .context("Timed out waiting for IPC response from swm-agent")?
        .context("Failed reading IPC response line")?;

    if read_res == 0 {
        bail!("IPC connection closed by swm-agent before sending response");
    }
    if line.len() > MAX_IPC_FRAME_BYTES {
        bail!("IPC response exceeds maximum frame size");
    }

    let resp: IpcResponse =
        serde_json::from_str(line.trim()).context("Invalid JSON in IPC response")?;
    Ok(resp)
}

/// Sends a single `IpcRequest` to the privileged `swm-agent` over Windows Named Pipe,
/// Unix Domain Socket, or loopback TCP (for non-admin tests).
pub async fn send_ipc_request(addr: &str, req: &IpcRequest) -> Result<IpcResponse> {
    if let Some(sock_addr) = parse_loopback_tcp_addr(addr)? {
        let stream = tokio::time::timeout(
            Duration::from_secs(5),
            tokio::net::TcpStream::connect(sock_addr),
        )
        .await
        .context("Timed out connecting to swm-agent loopback IPC")?
        .with_context(|| format!("Could not connect to swm-agent IPC at {sock_addr}"))?;
        return exchange_line(stream, req).await;
    }

    #[cfg(windows)]
    {
        if addr.trim().starts_with(r"\\.\pipe\") {
            use tokio::net::windows::named_pipe::ClientOptions;
            let pipe_name = addr.trim();
            let mut attempts = 0u32;
            let client = loop {
                match ClientOptions::new().open(pipe_name) {
                    Ok(c) => break c,
                    Err(e) if e.raw_os_error() == Some(231) && attempts < 20 => {
                        // ERROR_PIPE_BUSY (231) - wait briefly for next pipe instance
                        attempts += 1;
                        tokio::time::sleep(Duration::from_millis(50)).await;
                    }
                    Err(e) => {
                        return Err(e).with_context(|| {
                            format!("Could not open Windows Named Pipe '{pipe_name}'")
                        });
                    }
                }
            };
            return exchange_line(client, req).await;
        }
    }

    #[cfg(unix)]
    {
        let sock_path = addr.trim().strip_prefix("unix:").unwrap_or(addr.trim());
        let stream = tokio::time::timeout(
            Duration::from_secs(5),
            tokio::net::UnixStream::connect(sock_path),
        )
        .await
        .context("Timed out connecting to swm-agent Unix IPC socket")?
        .with_context(|| format!("Could not connect to Unix IPC socket '{sock_path}'"))?;
        return exchange_line(stream, req).await;
    }

    bail!("Unsupported IPC address '{}' for current OS platform", addr)
}
