use std::collections::{BTreeMap, HashMap};
use std::path::Path;
use swm_common::{compare_versions, compute_bytes_sha256, SoftwareEntry};

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

fn unescape_plist_xml_entities(raw: &str) -> String {
    if !raw.contains('&') {
        return raw.to_string();
    }
    raw.replace("&lt;", "<")
        .replace("&gt;", ">")
        .replace("&quot;", "\"")
        .replace("&apos;", "'")
        .replace("&amp;", "&")
}

/// Dependency-free XML plist top-level `<dict>` reader (`<key>...</key>` -> `<string>...</string>`).
/// Safely tracks `<dict>` and `<array>` nesting depths so nested sub-dictionaries (such as
/// `CFBundleDocumentTypes` or `CFBundleURLTypes`) never overwrite top-level bundle keys.
pub fn parse_plist_xml_dict(xml: &str) -> HashMap<String, String> {
    let mut map = HashMap::new();
    let mut dict_depth: usize = 0;
    let mut array_depth: usize = 0;
    let mut pending_key: Option<String> = None;
    let mut pos: usize = 0;

    while let Some(lt_rel) = xml[pos..].find('<') {
        let lt_idx = pos + lt_rel;
        let rest = &xml[lt_idx..];

        if rest.starts_with("<!--") {
            if let Some(end_comment) = rest.find("-->") {
                pos = lt_idx + end_comment + 3;
                continue;
            } else {
                break;
            }
        }
        if rest.starts_with("<![CDATA[") {
            if let Some(end_cdata) = rest.find("]]>") {
                pos = lt_idx + end_cdata + 3;
                continue;
            } else {
                break;
            }
        }
        if rest.starts_with("<?") {
            if let Some(end_pi) = rest.find("?>") {
                pos = lt_idx + end_pi + 2;
                continue;
            } else {
                break;
            }
        }
        if rest.starts_with("<!") {
            if let Some(end_decl) = rest.find('>') {
                pos = lt_idx + end_decl + 1;
                continue;
            } else {
                break;
            }
        }

        let Some(gt_rel) = rest.find('>') else {
            break;
        };
        let tag_raw = rest[1..gt_rel].trim();
        pos = lt_idx + gt_rel + 1;

        let is_closing = tag_raw.starts_with('/');
        let is_self_closing = !is_closing && tag_raw.ends_with('/');
        let tag_name = tag_raw
            .trim_start_matches('/')
            .trim_end_matches('/')
            .split_whitespace()
            .next()
            .unwrap_or("");

        match (tag_name, is_closing, is_self_closing) {
            ("dict", false, false) => {
                dict_depth += 1;
                if dict_depth > 1 {
                    pending_key = None;
                }
            }
            ("dict", true, _) => {
                dict_depth = dict_depth.saturating_sub(1);
                pending_key = None;
            }
            ("dict", false, true) => {
                if dict_depth <= 1 && array_depth == 0 {
                    pending_key = None;
                }
            }
            ("array", false, false) => {
                array_depth += 1;
                pending_key = None;
            }
            ("array", true, _) => {
                array_depth = array_depth.saturating_sub(1);
            }
            ("array", false, true) => {
                if dict_depth <= 1 && array_depth == 0 {
                    pending_key = None;
                }
            }
            ("key", false, false) if dict_depth <= 1 && array_depth == 0 => {
                if let Some(end_rel) = xml[pos..].find("</key>") {
                    let inner = unescape_plist_xml_entities(xml[pos..pos + end_rel].trim());
                    pending_key = Some(inner);
                    pos += end_rel + "</key>".len();
                } else {
                    break;
                }
            }
            ("string" | "integer" | "real", false, false)
                if dict_depth <= 1 && array_depth == 0 =>
            {
                let close_tag = format!("</{tag_name}>");
                if let Some(end_rel) = xml[pos..].find(&close_tag) {
                    let inner = unescape_plist_xml_entities(xml[pos..pos + end_rel].trim());
                    if let Some(k) = pending_key.take() {
                        map.insert(k, inner);
                    }
                    pos += end_rel + close_tag.len();
                } else {
                    break;
                }
            }
            ("string", false, true) if dict_depth <= 1 && array_depth == 0 => {
                if let Some(k) = pending_key.take() {
                    map.insert(k, String::new());
                }
            }
            ("true", false, true) if dict_depth <= 1 && array_depth == 0 => {
                if let Some(k) = pending_key.take() {
                    map.insert(k, "true".to_string());
                }
            }
            ("false", false, true) if dict_depth <= 1 && array_depth == 0 => {
                if let Some(k) = pending_key.take() {
                    map.insert(k, "false".to_string());
                }
            }
            ("data" | "date", false, _) if dict_depth <= 1 && array_depth == 0 => {
                pending_key = None;
            }
            _ => {}
        }
    }

    map
}

/// Reads `<app>/Contents/Info.plist` into a key-value map.
/// Supports XML plists on all OS platforms and transparently converts binary `bplist00` files via `/usr/bin/plutil` on macOS.
pub fn read_macos_info_plist_dict(plist_path: &Path) -> Option<HashMap<String, String>> {
    let bytes = std::fs::read(plist_path).ok()?;

    #[cfg(target_os = "macos")]
    if bytes.starts_with(b"bplist") {
        let out = std::process::Command::new("/usr/bin/plutil")
            .args(["-convert", "xml1", "-o", "-", &plist_path.to_string_lossy()])
            .output()
            .ok()?;
        if out.status.success() {
            let xml = String::from_utf8_lossy(&out.stdout);
            let map = parse_plist_xml_dict(&xml);
            return (!map.is_empty()).then_some(map);
        }
        return None;
    }

    let xml = String::from_utf8_lossy(&bytes);
    let map = parse_plist_xml_dict(&xml);
    (!map.is_empty()).then_some(map)
}

/// Parses a macOS `.app/Contents/Info.plist` XML document into a `SoftwareEntry` (`source = "app_bundle"`).
pub fn parse_macos_info_plist_xml(xml: &str) -> Option<SoftwareEntry> {
    let dict = parse_plist_xml_dict(xml);
    software_entry_from_info_plist_dict(&dict, None)
}

fn software_entry_from_info_plist_dict(
    dict: &HashMap<String, String>,
    fallback_app_name: Option<&str>,
) -> Option<SoftwareEntry> {
    let bundle_id = dict
        .get("CFBundleIdentifier")
        .map(|s| s.trim())
        .unwrap_or("")
        .to_string();

    let name = dict
        .get("CFBundleDisplayName")
        .map(|s| s.trim())
        .filter(|s| !s.is_empty())
        .or_else(|| {
            dict.get("CFBundleName")
                .map(|s| s.trim())
                .filter(|s| !s.is_empty())
        })
        .or_else(|| fallback_app_name.map(str::trim).filter(|s| !s.is_empty()))
        .or_else(|| {
            dict.get("CFBundleExecutable")
                .map(|s| s.trim())
                .filter(|s| !s.is_empty())
        })
        .unwrap_or(bundle_id.as_str())
        .to_string();

    if name.is_empty() {
        return None;
    }

    let version = dict
        .get("CFBundleShortVersionString")
        .map(|s| s.trim())
        .filter(|s| !s.is_empty())
        .or_else(|| {
            dict.get("CFBundleVersion")
                .map(|s| s.trim())
                .filter(|s| !s.is_empty())
        })?
        .to_string();

    let publisher = if bundle_id.starts_with("com.apple.") {
        "Apple".to_string()
    } else {
        String::new()
    };

    let package_ref = if !bundle_id.is_empty() {
        bundle_id
    } else {
        name.clone()
    };

    Some(SoftwareEntry {
        name,
        version,
        publisher,
        source: "app_bundle".to_string(),
        scope: "machine".to_string(),
        package_ref,
    })
}

/// Parses `pkgutil --pkgs` receipt list output (one receipt ID per line).
pub fn parse_pkgutil_pkgs_output(output: &str) -> Vec<String> {
    output
        .lines()
        .map(str::trim)
        .filter(|l| !l.is_empty())
        .map(ToString::to_string)
        .collect()
}

/// Parses `pkgutil --pkg-info <package_id>` output into `(package_id, version)`.
pub fn parse_pkgutil_pkg_info(output: &str) -> Option<(String, String)> {
    let mut pkg_id = String::new();
    let mut version = String::new();
    for line in output.lines() {
        let Some((k, v)) = line.split_once(':') else {
            continue;
        };
        match k.trim() {
            "package-id" => pkg_id = v.trim().to_string(),
            "version" => version = v.trim().to_string(),
            _ => {}
        }
    }
    if version.is_empty() {
        None
    } else {
        Some((pkg_id, version))
    }
}

/// Parses `brew list --formula --versions` (`source_type = "brew"`) or
/// `brew list --cask --versions` (`source_type = "brew_cask"`) output into `SoftwareEntry` rows.
pub fn parse_brew_list_versions(output: &str, source_type: &str) -> Vec<SoftwareEntry> {
    let mut entries = Vec::new();
    for line in output.lines() {
        let mut tokens = line.split_whitespace();
        let Some(pkg) = tokens.next() else {
            continue;
        };
        let versions: Vec<&str> = tokens.collect();
        if pkg.is_empty() || versions.is_empty() {
            continue;
        }
        let best_ver = versions
            .into_iter()
            .max_by(|a, b| compare_versions(a, b))
            .unwrap_or("0.0");

        entries.push(SoftwareEntry {
            name: pkg.to_string(),
            version: best_ver.to_string(),
            publisher: "Homebrew".to_string(),
            source: source_type.to_string(),
            scope: "machine".to_string(),
            package_ref: pkg.to_string(),
        });
    }
    entries
}

#[cfg(target_os = "macos")]
fn collect_macos_installed_software() -> Vec<SoftwareEntry> {
    let mut all: BTreeMap<(String, String), SoftwareEntry> = BTreeMap::new();

    // 1. /Applications/*.app/Contents/Info.plist
    if let Ok(entries) = std::fs::read_dir("/Applications") {
        for entry in entries.flatten() {
            let path = entry.path();
            let is_app = path
                .extension()
                .and_then(|e| e.to_str())
                .map(|s| s.eq_ignore_ascii_case("app"))
                .unwrap_or(false);
            if !is_app {
                continue;
            }
            let fallback_stem = path
                .file_stem()
                .and_then(|s| s.to_str())
                .unwrap_or("");
            let plist_path = path.join("Contents").join("Info.plist");
            if let Some(dict) = read_macos_info_plist_dict(&plist_path) {
                if let Some(sw) = software_entry_from_info_plist_dict(&dict, Some(fallback_stem)) {
                    all.insert((sw.source.clone(), sw.package_ref.clone()), sw);
                }
            }
        }
    }

    // 2. Third-party installer receipts via `pkgutil --pkgs`
    if let Ok(out) = std::process::Command::new("/usr/sbin/pkgutil")
        .arg("--pkgs")
        .output()
    {
        if out.status.success() {
            let pkg_ids = parse_pkgutil_pkgs_output(&String::from_utf8_lossy(&out.stdout));
            for pkg_id in pkg_ids
                .into_iter()
                .filter(|id| !id.starts_with("com.apple."))
                .take(150)
            {
                if let Ok(info_out) = std::process::Command::new("/usr/sbin/pkgutil")
                    .args(["--pkg-info", &pkg_id])
                    .output()
                {
                    if info_out.status.success() {
                        if let Some((_, ver)) =
                            parse_pkgutil_pkg_info(&String::from_utf8_lossy(&info_out.stdout))
                        {
                            all.insert(
                                ("pkgutil".to_string(), pkg_id.clone()),
                                SoftwareEntry {
                                    name: pkg_id.clone(),
                                    version: ver,
                                    publisher: String::new(),
                                    source: "pkgutil".to_string(),
                                    scope: "machine".to_string(),
                                    package_ref: pkg_id,
                                },
                            );
                        }
                    }
                }
            }
        }
    }

    // 3. Homebrew formulae & casks (executed strictly as the non-root Homebrew owner)
    if let Some(brew_bin) = crate::software::macos_installer::locate_brew_binary() {
        if let Ok(owner) = crate::software::macos_installer::resolve_brew_owner(&brew_bin) {
            let brew_str = brew_bin.to_string_lossy().to_string();
            for (flag, src_kind) in [("--formula", "brew"), ("--cask", "brew_cask")] {
                if let Ok(out) = std::process::Command::new("/usr/bin/sudo")
                    .args([
                        "-u",
                        &owner,
                        "-H",
                        "env",
                        "NONINTERACTIVE=1",
                        "HOMEBREW_NO_AUTO_UPDATE=1",
                        &brew_str,
                        "list",
                        flag,
                        "--versions",
                    ])
                    .current_dir("/tmp")
                    .output()
                {
                    if out.status.success() {
                        let stdout = String::from_utf8_lossy(&out.stdout);
                        for sw in parse_brew_list_versions(&stdout, src_kind) {
                            all.insert((sw.source.clone(), sw.package_ref.clone()), sw);
                        }
                    }
                }
            }
        }
    }

    all.into_values().collect()
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

#[cfg(all(unix, not(target_os = "macos")))]
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

    #[cfg(target_os = "macos")]
    {
        let mut entries = collect_macos_installed_software();
        entries.sort_by(|a, b| a.name.to_ascii_lowercase().cmp(&b.name.to_ascii_lowercase()));
        return entries;
    }

    #[cfg(all(unix, not(target_os = "macos")))]
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

    #[test]
    fn test_software_inv_parse_macos_info_plist_pkgutil_and_brew_versions() {
        let info_plist = r#"<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
<plist version="1.0">
<dict>
    <key>CFBundleURLTypes</key>
    <array>
        <dict>
            <key>CFBundleDisplayName</key>
            <string>Nested Ignored Name</string>
        </dict>
    </array>
    <key>CFBundleDisplayName</key>
    <string>Firefox &amp; Developer</string>
    <key>CFBundleName</key>
    <string>Firefox</string>
    <key>CFBundleIdentifier</key>
    <string>org.mozilla.firefox</string>
    <key>CFBundleShortVersionString</key>
    <string>132.0.2</string>
</dict>
</plist>"#;
        let sw = parse_macos_info_plist_xml(info_plist).expect("Expected valid SoftwareEntry");
        assert_eq!(sw.name, "Firefox & Developer");
        assert_eq!(sw.version, "132.0.2");
        assert_eq!(sw.source, "app_bundle");
        assert_eq!(sw.package_ref, "org.mozilla.firefox");

        let pkg_list = parse_pkgutil_pkgs_output("com.apple.pkg.Core\nus.zoom.pkg.videomeeting\n");
        assert_eq!(pkg_list.len(), 2);

        let pkg_info = "\
package-id: us.zoom.pkg.videomeeting
version: 6.2.6.41824
volume: /
location: Applications
";
        assert_eq!(
            parse_pkgutil_pkg_info(pkg_info),
            Some((
                "us.zoom.pkg.videomeeting".to_string(),
                "6.2.6.41824".to_string()
            ))
        );

        let brew_formulae = "git 2.47.0\nopenssl@3 3.3.2 3.4.0\n";
        let parsed_brew = parse_brew_list_versions(brew_formulae, "brew");
        assert_eq!(parsed_brew.len(), 2);
        assert_eq!(parsed_brew[0].name, "git");
        assert_eq!(parsed_brew[0].source, "brew");
        assert_eq!(parsed_brew[1].name, "openssl@3");
        assert_eq!(parsed_brew[1].version, "3.4.0");

        let brew_casks = "firefox 132.0.2\nvisual-studio-code 1.95.3\n";
        let parsed_casks = parse_brew_list_versions(brew_casks, "brew_cask");
        assert_eq!(parsed_casks.len(), 2);
        assert_eq!(parsed_casks[0].source, "brew_cask");
        assert_eq!(parsed_casks[0].package_ref, "firefox");
    }
}
