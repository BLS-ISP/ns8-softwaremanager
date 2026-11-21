#[cfg(unix)]
pub mod imp {
    use crate::ipc::{handle_ipc_stream, SharedAgentIpcContext};
    use anyhow::{Context, Result};
    use std::fs;
    use std::os::unix::fs::PermissionsExt;
    use std::path::Path;
    use tokio::net::UnixListener;
    use tokio::sync::watch;

    pub async fn serve_unix_domain_socket(
        sock_addr: String,
        ctx: SharedAgentIpcContext,
        mut shutdown_rx: watch::Receiver<bool>,
    ) -> Result<()> {
        let sock_path_str = sock_addr.trim().strip_prefix("unix:").unwrap_or(sock_addr.trim());
        let sock_path = Path::new(sock_path_str);

        if let Some(parent) = sock_path.parent() {
            fs::create_dir_all(parent).with_context(|| {
                format!("Failed creating IPC socket directory {}", parent.display())
            })?;
            let _ = fs::set_permissions(parent, fs::Permissions::from_mode(0o755));
        }

        if sock_path.exists() {
            let _ = fs::remove_file(sock_path);
        }

        let listener = UnixListener::bind(sock_path)
            .with_context(|| format!("Failed binding Unix IPC socket {}", sock_path.display()))?;
        let _ = fs::set_permissions(sock_path, fs::Permissions::from_mode(0o666));

        loop {
            tokio::select! {
                _ = shutdown_rx.changed() => {
                    if *shutdown_rx.borrow() {
                        break;
                    }
                }
                accept_res = listener.accept() => {
                    if let Ok((stream, _addr)) = accept_res {
                        let client_ctx = ctx.clone();
                        tokio::spawn(async move {
                            let _ = handle_ipc_stream(stream, client_ctx).await;
                        });
                    }
                }
            }
        }

        let _ = fs::remove_file(sock_path);
        Ok(())
    }
}
