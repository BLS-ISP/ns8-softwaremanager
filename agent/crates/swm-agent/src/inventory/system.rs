use std::collections::{BTreeSet, HashMap};
use std::fs;
use std::net::UdpSocket;
use swm_common::{compute_bytes_sha256, DiskInfo, HardwareInfo};
use sysinfo::{Disks, System};

#[derive(Debug, Clone)]
pub struct SystemSnapshot {
    pub hostname: String,
    pub domain: String,
    pub os_family: String,
    pub os_name: String,
    pub os_version: String,
    pub kernel_or_build: String,
    pub arch: String,
    pub machine_uid: String,
    pub hardware: HardwareInfo,
    pub ip_addresses: Vec<String>,
    pub logged_in_users: Vec<String>,
}

pub fn normalize_arch(raw_arch: &str) -> String {
    match raw_arch.trim().to_ascii_lowercase().as_str() {
        "x86_64" | "amd64" | "x64" => "amd64".to_string(),
        "aarch64" | "arm64" => "arm64".to_string(),
        "x86" | "i686" | "i386" => "x86".to_string(),
        other => other.to_string(),
    }
}

/// Parses Linux `/etc/os-release` key=value content cleanly.
pub fn parse_os_release(content: &str) -> HashMap<String, String> {
    let mut map = HashMap::new();
    for line in content.lines() {
        let trimmed = line.trim();
        if trimmed.is_empty() || trimmed.starts_with('#') {
            continue;
        }
        if let Some((k, v)) = trimmed.split_once('=') {
            let unquoted = v.trim().trim_matches('"').trim_matches('\'').to_string();
            map.insert(k.trim().to_string(), unquoted);
        }
    }
    map
}

pub fn detect_os_family() -> &'static str {
    if cfg!(windows) {
        "windows"
    } else {
        "linux"
    }
}

pub fn collect_machine_uid(hostname: &str) -> String {
    #[cfg(windows)]
    {
        use winreg::enums::{HKEY_LOCAL_MACHINE, KEY_READ};
        use winreg::RegKey;
        let hklm = RegKey::predef(HKEY_LOCAL_MACHINE);
        if let Ok(crypto) =
            hklm.open_subkey_with_flags(r"SOFTWARE\Microsoft\Cryptography", KEY_READ)
        {
            if let Ok(guid) = crypto.get_value::<String, _>("MachineGuid") {
                if !guid.trim().is_empty() {
                    return guid.trim().to_string();
                }
            }
        }
    }

    #[cfg(unix)]
    {
        for path in ["/etc/machine-id", "/var/lib/dbus/machine-id"] {
            if let Ok(id) = fs::read_to_string(path) {
                let trimmed = id.trim();
                if !trimmed.is_empty() {
                    return trimmed.to_string();
                }
            }
        }
    }

    format!("swm-{}", &compute_bytes_sha256(hostname.as_bytes())[..24])
}

fn detect_domain() -> String {
    if cfg!(windows) {
        if let Ok(dns_dom) = std::env::var("USERDNSDOMAIN") {
            if !dns_dom.trim().is_empty() {
                return dns_dom.trim().to_string();
            }
        }
        if let Ok(user_dom) = std::env::var("USERDOMAIN") {
            if !user_dom.trim().is_empty() {
                return user_dom.trim().to_string();
            }
        }
        return "WORKGROUP".to_string();
    }

    if let Ok(resolv) = fs::read_to_string("/etc/resolv.conf") {
        for line in resolv.lines() {
            let trimmed = line.trim();
            if let Some(rest) = trimmed
                .strip_prefix("domain ")
                .or_else(|| trimmed.strip_prefix("search "))
            {
                if let Some(first) = rest.split_whitespace().next() {
                    if !first.is_empty() && first != "." {
                        return first.to_string();
                    }
                }
            }
        }
    }
    "localdomain".to_string()
}

fn collect_logged_in_users() -> Vec<String> {
    let mut users = BTreeSet::new();

    #[cfg(unix)]
    {
        if let Ok(out) = std::process::Command::new("who").output() {
            if out.status.success() {
                let stdout = String::from_utf8_lossy(&out.stdout);
                for line in stdout.lines() {
                    if let Some(user) = line.split_whitespace().next() {
                        if !user.is_empty() {
                            users.insert(user.to_string());
                        }
                    }
                }
            }
        }
    }

    for env_key in ["USERNAME", "USER", "LOGNAME"] {
        if let Ok(val) = std::env::var(env_key) {
            let trimmed = val.trim();
            if !trimmed.is_empty()
                && !trimmed.eq_ignore_ascii_case("SYSTEM")
                && !trimmed.ends_with('$')
            {
                users.insert(trimmed.to_string());
            }
        }
    }

    users.into_iter().collect()
}

pub fn detect_hostname() -> String {
    System::host_name()
        .or_else(|| std::env::var("COMPUTERNAME").ok())
        .or_else(|| std::env::var("HOSTNAME").ok())
        .unwrap_or_else(|| "unknown-host".to_string())
}

/// Fast cross-platform check whether `swm-center(.exe)` is currently running in any user session.
/// Never fails or panics; returns `false` on any error.
pub fn detect_swm_center_running() -> bool {
    if let Ok(mock) = std::env::var("SWM_MOCK_CENTER_RUNNING") {
        match mock.trim() {
            "1" | "true" | "TRUE" => return true,
            "0" | "false" | "FALSE" => return false,
            _ => {}
        }
    }

    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        const CREATE_NO_WINDOW: u32 = 0x0800_0000;
        if let Ok(out) = std::process::Command::new("tasklist.exe")
            .creation_flags(CREATE_NO_WINDOW)
            .args(["/FI", "IMAGENAME eq swm-center.exe", "/NH"])
            .output()
        {
            if out.status.success() {
                let stdout = String::from_utf8_lossy(&out.stdout);
                return stdout.to_ascii_lowercase().contains("swm-center.exe");
            }
        }
        false
    }

    #[cfg(unix)]
    {
        if let Ok(out) = std::process::Command::new("pgrep")
            .args(["-x", "swm-center"])
            .output()
        {
            return out.status.success();
        }
        if let Ok(entries) = fs::read_dir("/proc") {
            for entry in entries.flatten() {
                let name = entry.file_name();
                let name_str = name.to_string_lossy();
                if !name_str.is_empty() && name_str.chars().all(|c| c.is_ascii_digit()) {
                    if let Ok(comm) = fs::read_to_string(entry.path().join("comm")) {
                        if comm.trim() == "swm-center" {
                            return true;
                        }
                    }
                }
            }
        }
        false
    }
}

pub fn collect_system_snapshot() -> SystemSnapshot {
    let mut sys = System::new();
    sys.refresh_cpu();
    sys.refresh_memory();

    let hostname = detect_hostname();

    let domain = detect_domain();
    let os_family = detect_os_family().to_string();

    let (mut os_name, mut os_version) = (
        System::name().unwrap_or_else(|| os_family.clone()),
        System::os_version().unwrap_or_else(|| "unknown".to_string()),
    );

    if !cfg!(windows) {
        if let Ok(os_rel_raw) = fs::read_to_string("/etc/os-release") {
            let parsed = parse_os_release(&os_rel_raw);
            if let Some(pretty) = parsed.get("PRETTY_NAME").or_else(|| parsed.get("NAME")) {
                os_name = pretty.clone();
            }
            if let Some(ver) = parsed.get("VERSION_ID").or_else(|| parsed.get("VERSION")) {
                os_version = ver.clone();
            }
        }
    }

    let kernel_or_build = System::kernel_version().unwrap_or_else(|| "unknown".to_string());
    let arch = normalize_arch(std::env::consts::ARCH);
    let machine_uid = collect_machine_uid(&hostname);

    let cpu_cores = sys.cpus().len().max(1) as u32;
    let cpu_model = sys
        .cpus()
        .first()
        .map(|c| c.brand().trim().to_string())
        .filter(|s| !s.is_empty())
        .unwrap_or_else(|| arch.clone());

    let ram_total_mb = sys.total_memory() / (1024 * 1024);
    let ram_free_mb = sys.available_memory() / (1024 * 1024);

    let disks_list = Disks::new_with_refreshed_list();
    let mut seen_mounts = BTreeSet::new();
    let mut disks = Vec::new();
    for d in disks_list.list() {
        let mount = d.mount_point().to_string_lossy().to_string();
        if d.total_space() == 0 || !seen_mounts.insert(mount.clone()) {
            continue;
        }
        let total_gb = ((d.total_space() as f64 / 1_073_741_824.0) * 10.0).round() / 10.0;
        let free_gb = ((d.available_space() as f64 / 1_073_741_824.0) * 10.0).round() / 10.0;
        disks.push(DiskInfo {
            mount,
            total_gb,
            free_gb,
        });
    }

    let mut ips = BTreeSet::new();
    // UDP routing table lookup (zero packets transmitted) for primary local IPv4 address
    if let Ok(sock) = UdpSocket::bind("0.0.0.0:0") {
        if sock.connect("192.0.2.1:80").is_ok() {
            if let Ok(local) = sock.local_addr() {
                let ip = local.ip();
                if !ip.is_loopback() && !ip.is_unspecified() {
                    ips.insert(ip.to_string());
                }
            }
        }
    }
    #[cfg(unix)]
    if let Ok(out) = std::process::Command::new("hostname").arg("-I").output() {
        if out.status.success() {
            for tok in String::from_utf8_lossy(&out.stdout).split_whitespace() {
                if !tok.starts_with("127.") && !tok.starts_with("fe80:") && tok != "::1" {
                    ips.insert(tok.to_string());
                }
            }
        }
    }

    let logged_in_users = collect_logged_in_users();

    SystemSnapshot {
        hostname,
        domain,
        os_family,
        os_name,
        os_version,
        kernel_or_build,
        arch,
        machine_uid,
        hardware: HardwareInfo {
            cpu_model,
            cpu_cores,
            ram_total_mb,
            ram_free_mb,
            disks,
        },
        ip_addresses: ips.into_iter().collect(),
        logged_in_users,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_inventory_parse_os_release_and_arch() {
        let sample = r#"
NAME="Rocky Linux"
VERSION="9.4 (Blue Onyx)"
ID="rocky"
VERSION_ID="9.4"
PRETTY_NAME="Rocky Linux 9.4 (Blue Onyx)"
"#;
        let map = parse_os_release(sample);
        assert_eq!(
            map.get("PRETTY_NAME").map(String::as_str),
            Some("Rocky Linux 9.4 (Blue Onyx)")
        );
        assert_eq!(map.get("VERSION_ID").map(String::as_str), Some("9.4"));
        assert_eq!(normalize_arch("x86_64"), "amd64");
        assert_eq!(normalize_arch("aarch64"), "arm64");
    }

    #[test]
    fn test_inventory_live_system_snapshot_populated() {
        let snap = collect_system_snapshot();
        assert!(!snap.hostname.is_empty());
        assert!(!snap.machine_uid.is_empty());
        assert!(snap.hardware.cpu_cores >= 1);
        assert!(snap.hardware.ram_total_mb > 0);
    }
}
