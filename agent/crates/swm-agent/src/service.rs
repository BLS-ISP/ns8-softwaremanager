use anyhow::{Context, Result};
use std::path::PathBuf;
use tokio::sync::watch;

pub const WINDOWS_SERVICE_NAME: &str = "NS8SoftwareManagerAgent";
pub const WINDOWS_SERVICE_DISPLAY: &str = "NS8 Software Manager Agent";
pub const SYSTEMD_UNIT_PATH: &str = "/etc/systemd/system/swm-agent.service";

/// Builds the exact `sc.exe` argument vectors used to configure Windows SCM crash recovery
/// (`restart/5000/restart/10000/restart/30000` + `failureflag 1`).
pub fn windows_scm_recovery_commands(service_name: &str) -> [Vec<String>; 2] {
    [
        vec![
            "failure".to_string(),
            service_name.to_string(),
            "reset=".to_string(),
            "86400".to_string(),
            "actions=".to_string(),
            "restart/5000/restart/10000/restart/30000".to_string(),
        ],
        vec![
            "failureflag".to_string(),
            service_name.to_string(),
            "1".to_string(),
        ],
    ]
}

/// Generates a clean systemd unit file content for Linux systems (`LF` line endings).
pub fn render_systemd_unit(exe_path: &str, data_dir: &str) -> String {
    format!(
        "[Unit]\n\
Description=NS8 Software Manager Agent\n\
After=network-online.target\n\
Wants=network-online.target\n\
\n\
[Service]\n\
Type=simple\n\
User=root\n\
ExecStart={exe_path} --data-dir {data_dir} run\n\
Restart=always\n\
RestartSec=10\n\
RuntimeDirectory=ns8-swm\n\
RuntimeDirectoryMode=0755\n\
\n\
[Install]\n\
WantedBy=multi-user.target\n"
    )
}

#[cfg(windows)]
pub fn install_os_service(data_dir: &PathBuf) -> Result<()> {
    use std::ffi::OsString;
    use windows_service::service::{
        ServiceAccess, ServiceErrorControl, ServiceInfo, ServiceStartType, ServiceType,
    };
    use windows_service::service_manager::{ServiceManager, ServiceManagerAccess};

    let manager_access = ServiceManagerAccess::CONNECT | ServiceManagerAccess::CREATE_SERVICE;
    let service_manager = ServiceManager::local_computer(None::<&str>, manager_access)
        .context("Failed connecting to Windows Service Control Manager (run as Administrator)")?;

    let current_exe = std::env::current_exe().context("Failed resolving swm-agent.exe path")?;
    let service_info = ServiceInfo {
        name: OsString::from(WINDOWS_SERVICE_NAME),
        display_name: OsString::from(WINDOWS_SERVICE_DISPLAY),
        service_type: ServiceType::OWN_PROCESS,
        start_type: ServiceStartType::AutoStart,
        error_control: ServiceErrorControl::Normal,
        executable_path: current_exe,
        launch_arguments: vec![
            OsString::from("--data-dir"),
            OsString::from(data_dir.as_os_str()),
            OsString::from("service-run"),
        ],
        dependencies: vec![],
        account_name: None, // LocalSystem (NT AUTHORITY\SYSTEM)
        account_password: None,
    };

    let service = service_manager
        .create_service(&service_info, ServiceAccess::CHANGE_CONFIG | ServiceAccess::START)
        .context("Failed creating NS8SoftwareManagerAgent Windows Service")?;
    let _ = service.set_description(
        "Managed OS patching, software deployment, and privileged IPC daemon for NethServer 8 Software Manager.",
    );

    {
        use std::os::windows::process::CommandExt;
        const CREATE_NO_WINDOW: u32 = 0x0800_0000;
        for args in windows_scm_recovery_commands(WINDOWS_SERVICE_NAME) {
            let _ = std::process::Command::new("sc.exe")
                .creation_flags(CREATE_NO_WINDOW)
                .args(&args)
                .output();
        }
    }

    Ok(())
}

#[cfg(windows)]
pub fn uninstall_os_service() -> Result<()> {
    use windows_service::service::ServiceAccess;
    use windows_service::service_manager::{ServiceManager, ServiceManagerAccess};

    let service_manager =
        ServiceManager::local_computer(None::<&str>, ServiceManagerAccess::CONNECT)
            .context("Failed connecting to Windows Service Control Manager")?;
    let service = service_manager
        .open_service(
            WINDOWS_SERVICE_NAME,
            ServiceAccess::STOP | ServiceAccess::DELETE | ServiceAccess::QUERY_STATUS,
        )
        .context("Failed opening NS8SoftwareManagerAgent service")?;
    let _ = service.stop();
    service
        .delete()
        .context("Failed deleting NS8SoftwareManagerAgent service")?;
    Ok(())
}

#[cfg(unix)]
pub fn install_os_service(data_dir: &PathBuf) -> Result<()> {
    let exe_path = std::env::current_exe().context("Failed resolving swm-agent binary path")?;
    let content = render_systemd_unit(
        &exe_path.to_string_lossy(),
        &data_dir.to_string_lossy(),
    );
    std::fs::write(SYSTEMD_UNIT_PATH, content.as_bytes())
        .with_context(|| format!("Failed writing {SYSTEMD_UNIT_PATH} (run as root)"))?;
    let _ = std::process::Command::new("systemctl")
        .args(["daemon-reload"])
        .status();
    let _ = std::process::Command::new("systemctl")
        .args(["enable", "swm-agent.service"])
        .status();
    Ok(())
}

#[cfg(unix)]
pub fn uninstall_os_service() -> Result<()> {
    let _ = std::process::Command::new("systemctl")
        .args(["disable", "--now", "swm-agent.service"])
        .status();
    if std::path::Path::new(SYSTEMD_UNIT_PATH).exists() {
        std::fs::remove_file(SYSTEMD_UNIT_PATH)?;
    }
    let _ = std::process::Command::new("systemctl")
        .args(["daemon-reload"])
        .status();
    Ok(())
}

#[cfg(windows)]
mod win_svc {
    use super::*;
    use std::ffi::OsString;
    use std::sync::OnceLock;
    use std::time::Duration;
    use windows_service::define_windows_service;
    use windows_service::service::{
        ServiceControl, ServiceControlAccept, ServiceExitCode, ServiceState, ServiceStatus,
        ServiceType,
    };
    use windows_service::service_control_handler::{self, ServiceControlHandlerResult};
    use windows_service::service_dispatcher;

    struct WinServiceParams {
        data_dir: PathBuf,
        ipc_addr: String,
    }

    static PARAMS: OnceLock<WinServiceParams> = OnceLock::new();

    define_windows_service!(ffi_service_main, windows_service_main);

    pub fn dispatch(data_dir: PathBuf, ipc_addr: String) -> Result<()> {
        let _ = PARAMS.set(WinServiceParams { data_dir, ipc_addr });
        service_dispatcher::start(WINDOWS_SERVICE_NAME, ffi_service_main)
            .context("Failed starting Windows Service dispatcher")
    }

    fn windows_service_main(_arguments: Vec<OsString>) {
        let (shutdown_tx, shutdown_rx) = watch::channel(false);
        let event_handler = move |control_event| -> ServiceControlHandlerResult {
            match control_event {
                ServiceControl::Stop | ServiceControl::Shutdown => {
                    let _ = shutdown_tx.send(true);
                    ServiceControlHandlerResult::NoError
                }
                ServiceControl::Interrogate => ServiceControlHandlerResult::NoError,
                _ => ServiceControlHandlerResult::NotImplemented,
            }
        };

        let Ok(status_handle) =
            service_control_handler::register(WINDOWS_SERVICE_NAME, event_handler)
        else {
            return;
        };

        let _ = status_handle.set_service_status(ServiceStatus {
            service_type: ServiceType::OWN_PROCESS,
            current_state: ServiceState::Running,
            controls_accepted: ServiceControlAccept::STOP | ServiceControlAccept::SHUTDOWN,
            exit_code: ServiceExitCode::Win32(0),
            checkpoint: 0,
            wait_hint: Duration::default(),
            process_id: None,
        });

        if let Some(params) = PARAMS.get() {
            if let Ok(rt) = tokio::runtime::Runtime::new() {
                let _ = rt.block_on(crate::daemon::run_daemon_loop(
                    params.data_dir.clone(),
                    params.ipc_addr.clone(),
                    30,
                    shutdown_rx,
                ));
            }
        }

        let _ = status_handle.set_service_status(ServiceStatus {
            service_type: ServiceType::OWN_PROCESS,
            current_state: ServiceState::Stopped,
            controls_accepted: ServiceControlAccept::empty(),
            exit_code: ServiceExitCode::Win32(0),
            checkpoint: 0,
            wait_hint: Duration::default(),
            process_id: None,
        });
    }
}

pub fn run_as_os_service(data_dir: PathBuf, ipc_addr: String) -> Result<()> {
    #[cfg(windows)]
    {
        return win_svc::dispatch(data_dir, ipc_addr);
    }
    #[cfg(not(windows))]
    {
        let rt = tokio::runtime::Runtime::new()?;
        let (shutdown_tx, shutdown_rx) = watch::channel(false);
        rt.spawn(async move {
            let _ = tokio::signal::ctrl_c().await;
            let _ = shutdown_tx.send(true);
        });
        rt.block_on(crate::daemon::run_daemon_loop(
            data_dir,
            ipc_addr,
            30,
            shutdown_rx,
        ))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_systemd_unit_generation_uses_lf_and_root() {
        let unit = render_systemd_unit("/usr/bin/swm-agent", "/etc/ns8-swm");
        assert!(!unit.contains("\r\n"));
        assert!(unit.contains("User=root"));
        assert!(unit.contains("ExecStart=/usr/bin/swm-agent --data-dir /etc/ns8-swm run"));
    }

    #[test]
    fn test_windows_scm_recovery_commands_match_spec() {
        let [failure_cmd, flag_cmd] = windows_scm_recovery_commands(WINDOWS_SERVICE_NAME);
        assert_eq!(
            failure_cmd.join(" "),
            "failure NS8SoftwareManagerAgent reset= 86400 actions= restart/5000/restart/10000/restart/30000"
        );
        assert_eq!(
            flag_cmd.join(" "),
            "failureflag NS8SoftwareManagerAgent 1"
        );
    }
}
