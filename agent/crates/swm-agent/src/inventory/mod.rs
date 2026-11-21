pub mod reboot;
pub mod software;
pub mod system;

use swm_common::{InventoryReport, PendingOsUpdate, SoftwareEntry, AGENT_VERSION};

pub struct CollectedInventory {
    pub report: InventoryReport,
    pub all_software: Vec<SoftwareEntry>,
    pub current_software_hash: String,
}

/// Collects a full cross-platform `InventoryReport`.
/// If `previous_software_hash` matches `current_software_hash` and `force_full_software` is `false`,
/// `installed_software` is omitted (`None`) so unchanged software lists are not re-transmitted.
pub fn build_inventory_report(
    previous_software_hash: Option<&str>,
    force_full_software: bool,
    pending_os_updates: Vec<PendingOsUpdate>,
    extra_reboot_required: bool,
) -> CollectedInventory {
    let sys = system::collect_system_snapshot();
    let os_reboot = reboot::detect_os_pending_reboot();
    let reboot_required = os_reboot || extra_reboot_required;

    let software = software::collect_installed_software();
    let software_hash = software::compute_software_delta_hash(&software);

    let unchanged = !force_full_software
        && previous_software_hash
            .map(|prev| prev == software_hash)
            .unwrap_or(false);

    let installed_software = if unchanged {
        None
    } else {
        Some(software.clone())
    };

    let report = InventoryReport {
        hostname: sys.hostname,
        domain: sys.domain,
        os_family: sys.os_family,
        os_name: sys.os_name,
        os_version: sys.os_version,
        kernel_or_build: sys.kernel_or_build,
        arch: sys.arch,
        agent_version: AGENT_VERSION.to_string(),
        hardware: sys.hardware,
        ip_addresses: sys.ip_addresses,
        logged_in_users: sys.logged_in_users,
        reboot_required,
        software_hash: Some(software_hash.clone()),
        installed_software,
        pending_os_updates,
    };

    CollectedInventory {
        report,
        all_software: software,
        current_software_hash: software_hash,
    }
}
