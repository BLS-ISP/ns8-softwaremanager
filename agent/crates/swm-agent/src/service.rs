use anyhow::{Context, Result};
use std::path::PathBuf;
use tokio::sync::watch;

pub const WINDOWS_SERVICE_NAME: &str = "NS8SoftwareManagerAgent";
pub const WINDOWS_SERVICE_DISPLAY: &str = "NS8 Software Manager Agent";
pub const SYSTEMD_UNIT_PATH: &str = "/etc/systemd/system/swm-agent.service";
pub const LAUNCHD_DAEMON_LABEL: &str = "org.nethserver.swm-agent";
pub const LAUNCHD_DAEMON_PLIST_PATH: &str = "/Library/LaunchDaemons/org.nethserver.swm-agent.plist";
pub const LAUNCHD_CENTER_LABEL: &str = "org.nethserver.swm-center";
pub const LAUNCHD_CENTER_PLIST_PATH: &str = "/Library/LaunchAgents/org.nethserver.swm-center.plist";

fn escape_plist_xml(s: &str) -> String {
    let mut out = String::with_capacity(s.len() + 8);
    for ch in s.chars() {
        match ch {
            '&' => out.push_str("&amp;"),
            '<' => out.push_str("&lt;"),
            '>' => out.push_str("&gt;"),
            '"' => out.push_str("&quot;"),
            '\'' => out.push_str("&apos;"),
            _ => out.push(ch),
        }
    }
    out
}

/// Generates a clean macOS `launchd` LaunchDaemon plist (`LF` line endings, `KeepAlive = true`).
pub fn render_launchd_daemon_plist(exe_path: &str, data_dir: &str) -> String {
    let exe_esc = escape_plist_xml(exe_path);
    let data_esc = escape_plist_xml(data_dir);
    let lines = [
        "<?xml version=\"1.0\" encoding=\"UTF-8\"?>".to_string(),
        "<!DOCTYPE plist PUBLIC \"-//Apple//DTD PLIST 1.0//EN\" \"http://www.apple.com/DTDs/PropertyList-1.0.dtd\">".to_string(),
        "<plist version=\"1.0\">".to_string(),
        "<dict>".to_string(),
        "    <key>Label</key>".to_string(),
        format!("    <string>{LAUNCHD_DAEMON_LABEL}</string>"),
        "    <key>ProgramArguments</key>".to_string(),
        "    <array>".to_string(),
        format!("        <string>{exe_esc}</string>"),
        "        <string>--data-dir</string>".to_string(),
        format!("        <string>{data_esc}</string>"),
        "        <string>run</string>".to_string(),
        "    </array>".to_string(),
        "    <key>RunAtLoad</key>".to_string(),
        "    <true/>".to_string(),
        "    <key>KeepAlive</key>".to_string(),
        "    <true/>".to_string(),
        "    <key>ThrottleInterval</key>".to_string(),
        "    <integer>10</integer>".to_string(),
        "    <key>StandardOutPath</key>".to_string(),
        "    <string>/var/log/ns8-swm-agent.log</string>".to_string(),
        "    <key>StandardErrorPath</key>".to_string(),
        "    <string>/var/log/ns8-swm-agent.err.log</string>".to_string(),
        "</dict>".to_string(),
        "</plist>".to_string(),
        String::new(),
    ];
    lines.join("\n")
}

/// Builds the `launchctl` argument vectors used by the macOS session watchdog to revive `swm-center`:
/// `(kickstart_args, bootstrap_args)`.
pub fn macos_watchdog_launchctl_commands(uid: u32) -> (Vec<String>, Vec<String>) {
    (
        vec![
            "kickstart".to_string(),
            "-k".to_string(),
            format!("gui/{uid}/{LAUNCHD_CENTER_LABEL}"),
        ],
        vec![
            "bootstrap".to_string(),
            format!("gui/{uid}"),
            LAUNCHD_CENTER_PLIST_PATH.to_string(),
        ],
    )
}

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

#[cfg(target_os = "macos")]
pub fn install_os_service(data_dir: &PathBuf) -> Result<()> {
    use std::os::unix::fs::PermissionsExt;

    let exe_path = std::env::current_exe().context("Failed resolving swm-agent binary path")?;
    std::fs::create_dir_all(data_dir)
        .with_context(|| format!("Failed creating data directory {}", data_dir.display()))?;
    let _ = std::fs::set_permissions(data_dir, std::fs::Permissions::from_mode(0o755));

    let ipc_dir = std::path::Path::new("/var/run/ns8-swm");
    let _ = std::fs::create_dir_all(ipc_dir);
    let _ = std::fs::set_permissions(ipc_dir, std::fs::Permissions::from_mode(0o755));

    let content = render_launchd_daemon_plist(
        &exe_path.to_string_lossy(),
        &data_dir.to_string_lossy(),
    );
    std::fs::write(LAUNCHD_DAEMON_PLIST_PATH, content.as_bytes()).with_context(|| {
        format!("Failed writing {LAUNCHD_DAEMON_PLIST_PATH} (run as root)")
    })?;
    let _ = std::fs::set_permissions(
        LAUNCHD_DAEMON_PLIST_PATH,
        std::fs::Permissions::from_mode(0o644),
    );

    let domain_target = format!("system/{LAUNCHD_DAEMON_LABEL}");
    let _ = std::process::Command::new("/bin/launchctl")
        .args(["bootout", &domain_target])
        .status();
    let boot = std::process::Command::new("/bin/launchctl")
        .args(["bootstrap", "system", LAUNCHD_DAEMON_PLIST_PATH])
        .status();
    if !boot.map(|s| s.success()).unwrap_or(false) {
        let _ = std::process::Command::new("/bin/launchctl")
            .args(["load", "-w", LAUNCHD_DAEMON_PLIST_PATH])
            .status();
    }
    let _ = std::process::Command::new("/bin/launchctl")
        .args(["enable", &domain_target])
        .status();
    Ok(())
}

#[cfg(target_os = "macos")]
pub fn uninstall_os_service() -> Result<()> {
    let domain_target = format!("system/{LAUNCHD_DAEMON_LABEL}");
    let bootout = std::process::Command::new("/bin/launchctl")
        .args(["bootout", &domain_target])
        .status();
    if !bootout.map(|s| s.success()).unwrap_or(false) {
        let _ = std::process::Command::new("/bin/launchctl")
            .args(["unload", "-w", LAUNCHD_DAEMON_PLIST_PATH])
            .status();
    }
    if std::path::Path::new(LAUNCHD_DAEMON_PLIST_PATH).exists() {
        std::fs::remove_file(LAUNCHD_DAEMON_PLIST_PATH)?;
    }
    Ok(())
}

#[cfg(all(unix, not(target_os = "macos")))]
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

#[cfg(all(unix, not(target_os = "macos")))]
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

    #[test]
    fn test_launchd_daemon_plist_and_watchdog_commands() {
        let plist = render_launchd_daemon_plist(
            "/usr/local/bin/swm-agent",
            "/Library/Application Support/NS8-SoftwareManager",
        );
        assert!(!plist.contains("\r\n"));
        assert!(plist.contains("<string>org.nethserver.swm-agent</string>"));
        assert!(plist.contains("<string>/usr/local/bin/swm-agent</string>"));
        assert!(plist.contains("<string>/Library/Application Support/NS8-SoftwareManager</string>"));
        assert!(plist.contains("<key>KeepAlive</key>\n    <true/>"));
        assert!(plist.contains("<key>RunAtLoad</key>\n    <true/>"));

        let (kick, boot) = macos_watchdog_launchctl_commands(501);
        assert_eq!(
            kick,
            vec!["kickstart", "-k", "gui/501/org.nethserver.swm-center"]
        );
        assert_eq!(
            boot,
            vec![
                "bootstrap",
                "gui/501",
                "/Library/LaunchAgents/org.nethserver.swm-center.plist"
            ]
        );
    }
}
