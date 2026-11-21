use std::collections::BTreeMap;
use swm_common::{compute_bytes_sha256, SoftwareEntry};

/// Computes a deterministic SHA-256 delta hash over the sorted installed software list
/// so unchanged software inventories are not needlessly re-transmitted.
pub fn compute_software_delta_hash(entries: &[SoftwareEntry]) -> String {
    let mut canonical_rows: Vec<String> = entries
        .iter()
        .map(|e| {
            format!(
                "{}|{}|{}|{}|{}|{}",
                e.name.trim(),
                e.version.trim(),
                e.publisher.trim(),
                e.source.trim(),
                e.scope.trim(),
                e.package_ref.trim()
            )
        })
        .collect();
    canonical_rows.sort();
    canonical_rows.dedup();
    compute_bytes_sha256(canonical_rows.join("\n").as_bytes())
}

/// Parses `dpkg-query -W -f='${Package}\t${Version}\t${Maintainer}\t${Status}\n'` output.
pub fn parse_dpkg_query_output(output: &str) -> Vec<SoftwareEntry> {
    let mut list = Vec::new();
    for line in output.lines() {
        let cols: Vec<&str> = line.split('\t').collect();
        if cols.len() < 4 {
            continue;
        }
        let pkg = cols[0].trim();
        let ver = cols[1].trim();
        let maintainer = cols[2].trim();
        let status = cols[3].trim();
        if pkg.is_empty() || ver.is_empty() || !status.contains("install ok installed") {
            continue;
        }
        list.push(SoftwareEntry {
            name: pkg.to_string(),
            version: ver.to_string(),
            publisher: maintainer.to_string(),
            source: "apt".to_string(),
            scope: "machine".to_string(),
            package_ref: pkg.to_string(),
        });
    }
    list
}

/// Parses `rpm -qa --queryformat '%{NAME}\t%{VERSION}-%{RELEASE}\t%{VENDOR}\n'` output.
pub fn parse_rpm_qa_output(output: &str) -> Vec<SoftwareEntry> {
    let mut list = Vec::new();
    for line in output.lines() {
        let cols: Vec<&str> = line.split('\t').collect();
        if cols.len() < 2 {
            continue;
        }
        let pkg = cols[0].trim();
        let ver = cols[1].trim();
        let vendor = cols.get(2).copied().unwrap_or("").trim();
        if pkg.is_empty() || ver.is_empty() || pkg == "gpg-pubkey" {
            continue;
        }
        let clean_vendor = if vendor == "(none)" { "" } else { vendor };
        list.push(SoftwareEntry {
            name: pkg.to_string(),
            version: ver.to_string(),
            publisher: clean_vendor.to_string(),
            source: "dnf".to_string(),
            scope: "machine".to_string(),
            package_ref: pkg.to_string(),
        });
    }
    list
}

/// Parses `flatpak list --app --columns=name,application,version,origin,installation` output.
pub fn parse_flatpak_list_output(output: &str) -> Vec<SoftwareEntry> {
    let mut list = Vec::new();
    for line in output.lines() {
        let cols: Vec<&str> = line.split('\t').collect();
        if cols.len() < 2 {
            continue;
        }
        let name = cols[0].trim();
        let app_id = cols[1].trim();
        if name.is_empty() || app_id.is_empty() || app_id.eq_ignore_ascii_case("Application ID") {
            continue;
        }
        let ver = cols.get(2).copied().unwrap_or("").trim();
        let origin = cols.get(3).copied().unwrap_or("flathub").trim();
        let installation = cols.get(4).copied().unwrap_or("system").trim();
        let scope = if installation.eq_ignore_ascii_case("user") {
            "user"
        } else {
            "machine"
        };
        list.push(SoftwareEntry {
            name: name.to_string(),
            version: if ver.is_empty() {
                "latest".to_string()
            } else {
                ver.to_string()
            },
            publisher: origin.to_string(),
            source: "flatpak".to_string(),
            scope: scope.to_string(),
            package_ref: app_id.to_string(),
        });
    }
    list
}

/// Parses `winget list` tabular output when available.
pub fn parse_winget_list_output(output: &str) -> Vec<SoftwareEntry> {
    let mut entries = Vec::new();
    let lines: Vec<&str> = output.lines().collect();
    let Some(sep_idx) = lines
        .iter()
        .position(|l| l.trim().starts_with("----") && l.len() >= 20)
    else {
        return entries;
    };
    if sep_idx == 0 {
        return entries;
    }

    let header = lines[sep_idx - 1];
    let id_col = header.find("Id").unwrap_or(0);
    let ver_col = header.find("Version").unwrap_or(0);
    if id_col == 0 || ver_col <= id_col {
        return entries;
    }
    let avail_col = header.find("Available");
    let source_col = header.find("Source");

    for row in &lines[(sep_idx + 1)..] {
        if row.len() < ver_col {
            continue;
        }
        if !row.is_char_boundary(id_col) || !row.is_char_boundary(ver_col) {
            continue;
        }
        let name = row[..id_col].trim();
        let id = row[id_col..ver_col].trim();
        let ver_end = avail_col
            .or(source_col)
            .filter(|& idx| idx > ver_col && idx <= row.len() && row.is_char_boundary(idx))
            .unwrap_or(row.len());
        let ver_chunk = row[ver_col..ver_end].trim();
        let version = ver_chunk.split_whitespace().next().unwrap_or("");
        if name.is_empty() || id.is_empty() || version.is_empty() {
            continue;
        }
        entries.push(SoftwareEntry {
            name: name.to_string(),
            version: version.to_string(),
            publisher: String::new(),
            source: "winget".to_string(),
            scope: "machine".to_string(),
            package_ref: id.to_string(),
        });
    }
    entries
}

#[cfg(windows)]
fn collect_windows_registry_software() -> Vec<SoftwareEntry> {
    use winreg::enums::{HKEY_CURRENT_USER, HKEY_LOCAL_MACHINE, KEY_READ};
    use winreg::RegKey;

    let mut dedup: BTreeMap<(String, String, String), SoftwareEntry> = BTreeMap::new();

    let targets = [
        (
            HKEY_LOCAL_MACHINE,
            r"SOFTWARE\Microsoft\Windows\CurrentVersion\Uninstall",
            "machine",
        ),
        (
            HKEY_LOCAL_MACHINE,
            r"SOFTWARE\WOW6432Node\Microsoft\Windows\CurrentVersion\Uninstall",
            "machine",
        ),
        (
            HKEY_CURRENT_USER,
            r"SOFTWARE\Microsoft\Windows\CurrentVersion\Uninstall",
            "user",
        ),
    ];

    for (hive, subkey_path, scope) in targets {
        let root = RegKey::predef(hive);
        let Ok(uninstall_key) = root.open_subkey_with_flags(subkey_path, KEY_READ) else {
            continue;
        };
        for key_name in uninstall_key.enum_keys().flatten() {
            let Ok(app_key) = uninstall_key.open_subkey_with_flags(&key_name, KEY_READ) else {
                continue;
            };
            // Skip system components if marked SystemComponent == 1
            if app_key.get_value::<u32, _>("SystemComponent").ok() == Some(1) {
                continue;
            }
            let Ok(display_name) = app_key.get_value::<String, _>("DisplayName") else {
                continue;
            };
            let name = display_name.trim().to_string();
            if name.is_empty() {
                continue;
            }
            let version = app_key
                .get_value::<String, _>("DisplayVersion")
                .unwrap_or_else(|_| "0.0".to_string())
                .trim()
                .to_string();
            let publisher = app_key
                .get_value::<String, _>("Publisher")
                .unwrap_or_default()
                .trim()
                .to_string();
            let is_msi = app_key.get_value::<u32, _>("WindowsInstaller").ok() == Some(1);
            let source = if is_msi { "msi" } else { "registry" }.to_string();

            dedup.insert(
                (name.to_ascii_lowercase(), version.clone(), scope.to_string()),
                SoftwareEntry {
                    name,
                    version,
                    publisher,
                    source,
                    scope: scope.to_string(),
                    package_ref: key_name,
                },
            );
        }
    }

    dedup.into_values().collect()
}

#[cfg(not(windows))]
fn collect_linux_installed_software() -> Vec<SoftwareEntry> {
    let mut all: BTreeMap<(String, String, String), SoftwareEntry> = BTreeMap::new();

    if let Ok(out) = std::process::Command::new("dpkg-query")
        .args(["-W", "-f=${Package}\t${Version}\t${Maintainer}\t${Status}\n"])
        .output()
    {
        if out.status.success() {
            for item in parse_dpkg_query_output(&String::from_utf8_lossy(&out.stdout)) {
                all.insert(
                    (item.source.clone(), item.package_ref.clone(), item.scope.clone()),
                    item,
                );
            }
        }
    }

    if all.is_empty() {
        if let Ok(out) = std::process::Command::new("rpm")
            .args(["-qa", "--queryformat", "%{NAME}\t%{VERSION}-%{RELEASE}\t%{VENDOR}\n"])
            .output()
        {
            if out.status.success() {
                for item in parse_rpm_qa_output(&String::from_utf8_lossy(&out.stdout)) {
                    all.insert(
                        (item.source.clone(), item.package_ref.clone(), item.scope.clone()),
                        item,
                    );
                }
            }
        }
    }

    if let Ok(out) = std::process::Command::new("flatpak")
        .args([
            "list",
            "--app",
            "--columns=name,application,version,origin,installation",
        ])
        .output()
    {
        if out.status.success() {
            for item in parse_flatpak_list_output(&String::from_utf8_lossy(&out.stdout)) {
                all.insert(
                    (item.source.clone(), item.package_ref.clone(), item.scope.clone()),
                    item,
                );
            }
        }
    }

    all.into_values().collect()
}

pub fn collect_installed_software() -> Vec<SoftwareEntry> {
    if let Ok(mock_json) = std::env::var("SWM_MOCK_SOFTWARE_JSON") {
        if let Ok(parsed) = serde_json::from_str::<Vec<SoftwareEntry>>(&mock_json) {
            return parsed;
        }
    }

    #[cfg(windows)]
    {
        let mut entries = collect_windows_registry_software();
        let _ = BTreeMap::<u8, u8>::new();
        entries.sort_by(|a, b| a.name.to_ascii_lowercase().cmp(&b.name.to_ascii_lowercase()));
        return entries;
    }

    #[cfg(not(windows))]
    {
        let mut entries = collect_linux_installed_software();
        entries.sort_by(|a, b| a.name.to_ascii_lowercase().cmp(&b.name.to_ascii_lowercase()));
        return entries;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_software_inv_parse_dpkg_rpm_flatpak_and_winget() {
        let dpkg_sample = "curl\t8.5.0-2ubuntu10\tUbuntu Developers\tinstall ok installed\nremoved-pkg\t1.0\tMaint\tdeinstall ok config-files\n";
        let dpkg = parse_dpkg_query_output(dpkg_sample);
        assert_eq!(dpkg.len(), 1);
        assert_eq!(dpkg[0].name, "curl");
        assert_eq!(dpkg[0].source, "apt");

        let rpm_sample = "bash\t5.1.8-9.el9\tRocky Enterprise Software Foundation\ngpg-pubkey\tabc-123\t(none)\n";
        let rpm = parse_rpm_qa_output(rpm_sample);
        assert_eq!(rpm.len(), 1);
        assert_eq!(rpm[0].name, "bash");
        assert_eq!(rpm[0].source, "dnf");

        let flatpak_sample = "Firefox\torg.mozilla.firefox\t132.0\tflathub\tsystem\nVLC\torg.videolan.VLC\t3.0.21\tflathub\tuser\n";
        let fp = parse_flatpak_list_output(flatpak_sample);
        assert_eq!(fp.len(), 2);
        assert_eq!(fp[0].scope, "machine");
        assert_eq!(fp[1].scope, "user");

        let winget_sample = "\
Name               Id                  Version  Available Source
-----------------------------------------------------------------
7-Zip 24.08 (x64)  7zip.7zip           24.08              winget
Mozilla Firefox    Mozilla.Firefox     131.0    132.0     winget
";
        let wg = parse_winget_list_output(winget_sample);
        assert_eq!(wg.len(), 2);
        assert_eq!(wg[0].package_ref, "7zip.7zip");
        assert_eq!(wg[0].version, "24.08");
        assert_eq!(wg[1].package_ref, "Mozilla.Firefox");
        assert_eq!(wg[1].version, "131.0");
    }

    #[test]
    fn test_software_inv_delta_hash_stability() {
        let a = SoftwareEntry {
            name: "7-Zip".to_string(),
            version: "24.08".to_string(),
            publisher: "Igor Pavlov".to_string(),
            source: "msi".to_string(),
            scope: "machine".to_string(),
            package_ref: "7zip.7zip".to_string(),
        };
        let b = SoftwareEntry {
            name: "Firefox".to_string(),
            version: "132.0".to_string(),
            publisher: "Mozilla".to_string(),
            source: "winget".to_string(),
            scope: "machine".to_string(),
            package_ref: "Mozilla.Firefox".to_string(),
        };

        let h1 = compute_software_delta_hash(&[a.clone(), b.clone()]);
        let h2 = compute_software_delta_hash(&[b, a.clone()]);
        assert_eq!(h1, h2);

        let mut a_updated = a;
        a_updated.version = "24.09".to_string();
        let h3 = compute_software_delta_hash(&[a_updated]);
        assert_ne!(h1, h3);
    }
}
