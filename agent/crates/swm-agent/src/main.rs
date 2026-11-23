use anyhow::Result;
use clap::{Parser, Subcommand};
use std::path::PathBuf;
use swm_agent::config::default_data_dir;
use swm_agent::daemon::{enroll_device, run_daemon_loop, run_once, CycleOutcome};
use swm_agent::service::{install_os_service, run_as_os_service, uninstall_os_service};
use swm_common::default_ipc_address;
use tokio::sync::watch;

#[derive(Debug, Parser)]
#[command(
    name = "swm-agent",
    version,
    about = "NS8-SoftwareManager Privileged Endpoint Agent"
)]
struct Cli {
    /// Directory for persistent agent config, state, and package staging
    #[arg(long, global = true, env = "SWM_DATA_DIR")]
    data_dir: Option<PathBuf>,

    /// Local IPC endpoint address (Named Pipe on Windows, Unix Socket on macOS/Linux, or 127.0.0.1:<port> for tests)
    #[arg(long, global = true, env = "SWM_IPC_ADDR")]
    ipc_addr: Option<String>,

    #[command(subcommand)]
    command: Commands,
}

#[derive(Debug, Subcommand)]
enum Commands {
    /// Enroll this endpoint with the central NS8-SoftwareManager server
    Enroll {
        #[arg(long, env = "SWM_SERVER_URL")]
        server: String,

        #[arg(long, env = "SWM_ENROLLMENT_TOKEN")]
        token: String,

        /// Block until an administrator approves the device if status is `pending`
        #[arg(long, default_value_t = false)]
        wait_approval: bool,
    },

    /// Run the agent in foreground (or execute a single deterministic sync cycle with `--once`)
    Run {
        /// Run a single status + inventory + pending-jobs cycle and exit
        #[arg(long, default_value_t = false)]
        once: bool,

        /// Polling fallback interval in seconds
        #[arg(long, default_value_t = 30)]
        poll_interval_secs: u64,
    },

    /// Register `swm-agent` as a native Windows Service, macOS LaunchDaemon, or Linux systemd service
    ServiceInstall,

    /// Remove the registered Windows Service, macOS LaunchDaemon, or Linux systemd service
    ServiceUninstall,

    /// Internal entrypoint invoked by the OS service manager
    ServiceRun,
}

fn init_logging() {
    let filter = tracing_subscriber::EnvFilter::try_from_default_env()
        .unwrap_or_else(|_| tracing_subscriber::EnvFilter::new("info"));
    let _ = tracing_subscriber::fmt()
        .with_env_filter(filter)
        .with_target(false)
        .try_init();
}

fn main() -> Result<()> {
    init_logging();
    let cli = Cli::parse();
    let data_dir = cli.data_dir.unwrap_or_else(default_data_dir);
    let ipc_addr = cli
        .ipc_addr
        .unwrap_or_else(|| default_ipc_address().to_string());

    match cli.command {
        Commands::ServiceInstall => {
            install_os_service(&data_dir)?;
            println!("Service installed successfully (data_dir={}).", data_dir.display());
            Ok(())
        }
        Commands::ServiceUninstall => {
            uninstall_os_service()?;
            println!("Service uninstalled successfully.");
            Ok(())
        }
        Commands::ServiceRun => run_as_os_service(data_dir, ipc_addr),
        Commands::Enroll {
            server,
            token,
            wait_approval,
        } => {
            let rt = tokio::runtime::Runtime::new()?;
            let cfg = rt.block_on(enroll_device(&data_dir, &server, &token, wait_approval))?;
            println!(
                "Enrolled device_id={} status={} data_dir={}",
                cfg.device_id,
                cfg.status,
                data_dir.display()
            );
            Ok(())
        }
        Commands::Run {
            once,
            poll_interval_secs,
        } => {
            let rt = tokio::runtime::Runtime::new()?;
            if once {
                let outcome = rt.block_on(run_once(data_dir))?;
                match outcome {
                    CycleOutcome::PendingApproval => {
                        println!("status=pending");
                    }
                    CycleOutcome::Revoked => {
                        eprintln!("status=revoked");
                        std::process::exit(3);
                    }
                    CycleOutcome::Completed { jobs_executed } => {
                        println!("status=ok jobs_executed={jobs_executed}");
                    }
                }
                return Ok(());
            }

            let (shutdown_tx, shutdown_rx) = watch::channel(false);
            rt.spawn(async move {
                let _ = tokio::signal::ctrl_c().await;
                let _ = shutdown_tx.send(true);
            });
            rt.block_on(run_daemon_loop(
                data_dir,
                ipc_addr,
                poll_interval_secs,
                shutdown_rx,
            ))
        }
    }
}
