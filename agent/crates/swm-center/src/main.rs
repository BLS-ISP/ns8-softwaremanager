use anyhow::{bail, Context, Result};
use chrono::Utc;
use clap::{Parser, Subcommand};
use std::net::SocketAddr;
use std::sync::{Arc, RwLock};
use std::time::Duration;
use swm_center::bridge::{
    build_center_router, cleanup_session_file_if_owned, default_session_file_path,
    discover_ui_dist_dir, try_handoff_to_existing_instance, write_session_file, CenterBridgeState,
};
use swm_center::tray::{
    dispatch_native_notification, launch_desktop_window, should_spawn_native_tray,
    DesktopNotification, NativeTrayHandle, TrayMenuAction, TrayStateTracker,
};
use swm_common::{
    default_ipc_address, send_ipc_request, DesktopPolicy, IpcRequest, IpcResponseData,
    PackageAction, RebootState,
};

#[derive(Debug, Parser)]
#[command(
    name = "swm-center",
    version,
    about = "NS8 Software Center (Unprivileged End-User Desktop & Tray Client)"
)]
struct Cli {
    /// Local IPC endpoint address of `swm-agent`
    #[arg(long, global = true, env = "SWM_IPC_ADDR")]
    ipc_addr: Option<String>,

    #[command(subcommand)]
    command: Option<Commands>,
}

#[derive(Debug, Subcommand)]
enum Commands {
    /// Start the Software Center desktop UI bridge & System Tray watcher
    Gui {
        /// Loopback port for the local Software Center UI (0 = auto-select free port)
        #[arg(long, default_value_t = 0)]
        port: u16,

        /// Start minimized in System Tray without immediately opening a window
        #[arg(long, default_value_t = false)]
        tray_only: bool,

        /// Do not auto-launch browser/WebView window or Win32 tray handles (useful for automated tests)
        #[arg(long, default_value_t = false)]
        no_open: bool,
    },

    /// List available self-service catalog items via local IPC
    Catalog,

    /// Query current agent, update, and reboot status via local IPC
    Status,

    /// Trigger an authorized self-service catalog action (`install` | `update` | `uninstall`)
    Trigger {
        #[arg(long)]
        package_id: String,

        #[arg(long, default_value = "install")]
        action: String,
    },

    /// Proactively trigger pending approved OS/software updates via local IPC
    TriggerUpdates,

    /// Snooze a pending grace-period reboot reminder (`minutes`)
    Snooze {
        #[arg(long, default_value_t = 60)]
        minutes: u32,
    },

    /// Confirm and execute a pending system reboot immediately
    ConfirmReboot,
}

fn parse_action(raw: &str) -> Result<PackageAction> {
    match raw.trim().to_ascii_lowercase().as_str() {
        "install" => Ok(PackageAction::Install),
        "update" | "upgrade" => Ok(PackageAction::Update),
        "uninstall" | "remove" => Ok(PackageAction::Uninstall),
        other => bail!("Invalid action '{other}' (expected install, update, or uninstall)"),
    }
}

async fn run_gui_and_tray(
    ipc_addr: String,
    port: u16,
    tray_only: bool,
    no_open: bool,
) -> Result<()> {
    let single_instance_enabled = port == 0
        && !no_open
        && std::env::var("SWM_DISABLE_SINGLE_INSTANCE")
            .ok()
            .as_deref()
            != Some("1");
    let session_path = default_session_file_path();

    if single_instance_enabled
        && try_handoff_to_existing_instance(&session_path, tray_only, no_open).await
    {
        tracing::info!(
            "NS8 Software Center is already running in this user session; handed off and exiting."
        );
        return Ok(());
    }

    let bind_addr = SocketAddr::from(([127, 0, 0, 1], port));
    let listener = tokio::net::TcpListener::bind(bind_addr)
        .await
        .with_context(|| format!("Failed binding local Software Center UI on {bind_addr}"))?;
    let local_addr = listener.local_addr()?;
    let url = format!("http://{local_addr}");

    if single_instance_enabled {
        let _ = write_session_file(&session_path, &url);
    }

    let app = build_center_router(CenterBridgeState {
        ipc_addr: ipc_addr.clone(),
        ui_dist_dir: discover_ui_dist_dir(),
        local_url: url.clone(),
        no_open,
        ..Default::default()
    });

    println!("NS8 Software Center listening on {url}");

    let shared_reboot = Arc::new(RwLock::new(RebootState::default()));
    let shared_policy = Arc::new(RwLock::new(DesktopPolicy::default()));
    let (tray_action_tx, mut tray_action_rx) =
        tokio::sync::mpsc::unbounded_channel::<TrayMenuAction>();
    let (shutdown_tx, mut shutdown_rx) = tokio::sync::watch::channel(false);

    let native_tray = if should_spawn_native_tray(no_open) {
        NativeTrayHandle::spawn(
            url.clone(),
            Arc::clone(&shared_reboot),
            Arc::clone(&shared_policy),
            tray_action_tx.clone(),
        )
        .map(Arc::new)
    } else {
        None
    };

    if !tray_only && !no_open {
        launch_desktop_window(&url);
    }

    // Handle interactive System Tray menu actions over IPC
    let action_ipc = ipc_addr.clone();
    let action_url = url.clone();
    let action_reboot = Arc::clone(&shared_reboot);
    let action_policy = Arc::clone(&shared_policy);
    let action_shutdown_tx = shutdown_tx.clone();
    tokio::spawn(async move {
        while let Some(action) = tray_action_rx.recv().await {
            match action {
                TrayMenuAction::OpenSoftwareCenter | TrayMenuAction::ShowStatus => {
                    launch_desktop_window(&action_url);
                }
                TrayMenuAction::SyncNow => {
                    match send_ipc_request(&action_ipc, &IpcRequest::SyncNow).await {
                        Ok(resp) if resp.ok => {
                            let silent = action_policy
                                .read()
                                .map(|p| p.silent_notifications)
                                .unwrap_or(false);
                            if !silent {
                                dispatch_native_notification(&DesktopNotification {
                                    title: "NS8 Software Center".to_string(),
                                    body: "Synchronisation mit dem NS8-Server wurde gestartet."
                                        .to_string(),
                                    urgent: false,
                                });
                            }
                        }
                        Ok(resp) => {
                            tracing::warn!(
                                "Tray SyncNow returned non-ok response: {:?}",
                                resp.message
                            );
                        }
                        Err(err) => {
                            tracing::warn!("Tray SyncNow IPC failed: {err}");
                        }
                    }
                }
                TrayMenuAction::CheckForUpdates => {
                    match send_ipc_request(&action_ipc, &IpcRequest::TriggerPendingUpdates).await {
                        Ok(resp) if resp.ok => {
                            dispatch_native_notification(&DesktopNotification {
                                title: "NS8 Software Center".to_string(),
                                body: "Update-Prüfung und Installation ausstehender Updates wurde gestartet."
                                    .to_string(),
                                urgent: false,
                            });
                        }
                        Ok(resp) => {
                            let msg = resp
                                .message
                                .unwrap_or_else(|| "Keine ausstehenden Updates gefunden.".to_string());
                            dispatch_native_notification(&DesktopNotification {
                                title: "NS8 Software Center".to_string(),
                                body: msg,
                                urgent: false,
                            });
                        }
                        Err(err) => {
                            tracing::warn!("Tray CheckForUpdates IPC failed: {err}");
                        }
                    }
                }
                TrayMenuAction::SnoozeRebootMinutes(minutes) => {
                    if let Ok(resp) =
                        send_ipc_request(&action_ipc, &IpcRequest::SnoozeReboot { minutes }).await
                    {
                        if let Some(IpcResponseData::RebootUpdated(rb)) = resp.data {
                            if let Ok(mut guard) = action_reboot.write() {
                                *guard = rb;
                            }
                        }
                    }
                }
                TrayMenuAction::RebootNow => {
                    let _ = send_ipc_request(&action_ipc, &IpcRequest::ConfirmRebootNow).await;
                }
                TrayMenuAction::QuitTray => {
                    let enforce = action_policy
                        .read()
                        .map(|p| p.enforce_tray)
                        .unwrap_or(true);
                    if enforce {
                        tracing::info!(
                            "Ignored QuitTray action because enforce_tray policy is active."
                        );
                        continue;
                    }
                    let _ = action_shutdown_tx.send(true);
                    break;
                }
            }
        }
    });

    // Spawn background System Tray & Reboot Notification polling task
    let tray_ipc = ipc_addr.clone();
    let poll_reboot = Arc::clone(&shared_reboot);
    let poll_policy = Arc::clone(&shared_policy);
    let poll_tray_handle = native_tray.clone();
    tokio::spawn(async move {
        let mut tracker = TrayStateTracker::new(tray_only);
        let mut ticker = tokio::time::interval(Duration::from_secs(15));
        loop {
            ticker.tick().await;
            if let Ok(resp) = send_ipc_request(&tray_ipc, &IpcRequest::GetStatus).await {
                if let Some(IpcResponseData::Status(snap)) = resp.data {
                    if let Ok(mut guard) = poll_reboot.write() {
                        *guard = snap.reboot_state.clone();
                    }
                    if let Ok(mut pguard) = poll_policy.write() {
                        *pguard = snap.desktop_policy.clone();
                    }
                    for note in tracker.evaluate_notifications(&snap, Utc::now().timestamp()) {
                        dispatch_native_notification(&note);
                        if let Some(tray) = poll_tray_handle.as_ref() {
                            if note.urgent {
                                tray.show_balloon(note);
                            }
                        }
                    }
                }
            }
        }
    });

    let cleanup_url = url.clone();
    axum::serve(listener, app)
        .with_graceful_shutdown(async move {
            tokio::select! {
                _ = tokio::signal::ctrl_c() => {}
                _ = shutdown_rx.changed() => {}
            }
            if single_instance_enabled {
                cleanup_session_file_if_owned(&session_path, &cleanup_url);
            }
            if let Some(tray) = native_tray.as_ref() {
                tray.shutdown();
            }
        })
        .await
        .context("Software Center UI bridge error")?;
    Ok(())
}

#[tokio::main]
async fn main() -> Result<()> {
    let filter = tracing_subscriber::EnvFilter::try_from_default_env()
        .unwrap_or_else(|_| tracing_subscriber::EnvFilter::new("info"));
    let _ = tracing_subscriber::fmt()
        .with_env_filter(filter)
        .with_target(false)
        .try_init();

    let cli = Cli::parse();
    let ipc_addr = cli
        .ipc_addr
        .unwrap_or_else(|| default_ipc_address().to_string());

    let cmd = cli.command.unwrap_or(Commands::Gui {
        port: 0,
        tray_only: false,
        no_open: false,
    });

    match cmd {
        Commands::Gui {
            port,
            tray_only,
            no_open,
        } => run_gui_and_tray(ipc_addr, port, tray_only, no_open).await,

        Commands::Catalog => {
            let resp = send_ipc_request(&ipc_addr, &IpcRequest::ListCatalog).await?;
            println!("{}", serde_json::to_string_pretty(&resp)?);
            if !resp.ok {
                std::process::exit(1);
            }
            Ok(())
        }

        Commands::Status => {
            let resp = send_ipc_request(&ipc_addr, &IpcRequest::GetStatus).await?;
            println!("{}", serde_json::to_string_pretty(&resp)?);
            if !resp.ok {
                std::process::exit(1);
            }
            Ok(())
        }

        Commands::Trigger { package_id, action } => {
            let act = parse_action(&action)?;
            let resp = send_ipc_request(
                &ipc_addr,
                &IpcRequest::TriggerPackage {
                    package_id,
                    action: act,
                },
            )
            .await?;
            println!("{}", serde_json::to_string_pretty(&resp)?);
            if !resp.ok {
                std::process::exit(1);
            }
            Ok(())
        }

        Commands::TriggerUpdates => {
            let resp = send_ipc_request(&ipc_addr, &IpcRequest::TriggerPendingUpdates).await?;
            println!("{}", serde_json::to_string_pretty(&resp)?);
            if !resp.ok {
                std::process::exit(1);
            }
            Ok(())
        }

        Commands::Snooze { minutes } => {
            let resp =
                send_ipc_request(&ipc_addr, &IpcRequest::SnoozeReboot { minutes }).await?;
            println!("{}", serde_json::to_string_pretty(&resp)?);
            if !resp.ok {
                std::process::exit(1);
            }
            Ok(())
        }

        Commands::ConfirmReboot => {
            let resp = send_ipc_request(&ipc_addr, &IpcRequest::ConfirmRebootNow).await?;
            println!("{}", serde_json::to_string_pretty(&resp)?);
            if !resp.ok {
                std::process::exit(1);
            }
            Ok(())
        }
    }
}
