use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use std::collections::HashSet;
use std::time::Duration;
use swm_common::PendingOsUpdate;
use tokio::process::Command;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PatchExecutionOutcome {
    pub succeeded: bool,
    pub exit_code: i32,
    pub reboot_required: bool,
    pub installed_count: usize,
    pub stdout: String,
    pub stderr: String,
}

pub fn normalize_kb_id(raw: &str) -> String {
    let trimmed = raw.trim().to_ascii_uppercase();
    if trimmed.is_empty() {
        return String::new();
    }
    if trimmed.starts_with("KB") {
        trimmed
    } else if trimmed.chars().all(|c| c.is_ascii_digit()) {
        format!("KB{trimmed}")
    } else {
        trimmed
    }
}

pub fn is_kb_excluded(update_id: &str, title: &str, excluded_updates: &[String]) -> bool {
    if excluded_updates.is_empty() {
        return false;
    }
    let excluded_set: HashSet<String> = excluded_updates
        .iter()
        .map(|s| normalize_kb_id(s))
        .filter(|s| !s.is_empty())
        .collect();

    let norm_id = normalize_kb_id(update_id);
    if excluded_set.contains(&norm_id) {
        return true;
    }
    let upper_title = title.to_ascii_uppercase();
    excluded_set
        .iter()
        .any(|kb| kb.starts_with("KB") && upper_title.contains(kb))
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct RawWuaScanEntry {
    #[serde(default)]
    pub update_id: String,
    #[serde(default)]
    pub kb: String,
    #[serde(default)]
    pub title: String,
    #[serde(default)]
    pub severity: String,
    #[serde(default)]
    pub category: String,
    #[serde(default)]
    pub size_bytes: u64,
    #[serde(default)]
    pub reboot_behavior: u32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct RawWuaInstallResult {
    #[serde(default)]
    pub result_code: i32,
    #[serde(default)]
    pub reboot_required: bool,
    #[serde(default)]
    pub installed_count: usize,
    #[serde(default)]
    pub message: String,
}

pub fn parse_wua_scan_json(
    raw_json: &str,
    excluded_updates: &[String],
) -> Result<Vec<PendingOsUpdate>> {
    let trimmed = raw_json.trim();
    if trimmed.is_empty() || trimmed == "null" || trimmed == "[]" {
        return Ok(Vec::new());
    }

    // PowerShell ConvertTo-Json emits a single object when count == 1, or an array when count > 1
    let val: serde_json::Value =
        serde_json::from_str(trimmed).context("Invalid JSON returned from WUA scan")?;
    let items: Vec<RawWuaScanEntry> = match val {
        serde_json::Value::Array(_) => serde_json::from_value(val)?,
        serde_json::Value::Object(_) => vec![serde_json::from_value(val)?],
        _ => Vec::new(),
    };

    let mut out = Vec::new();
    for entry in items {
        let effective_id = if !entry.kb.trim().is_empty() {
            normalize_kb_id(&entry.kb)
        } else {
            entry.update_id.trim().to_string()
        };
        if effective_id.is_empty() {
            continue;
        }
        if is_kb_excluded(&effective_id, &entry.title, excluded_updates) {
            continue;
        }

        let cat_lower = entry.category.to_ascii_lowercase();
        let category = if cat_lower.contains("security") {
            "security"
        } else if cat_lower.contains("driver") {
            "drivers"
        } else if cat_lower.contains("critical") {
            "security"
        } else {
            "updates"
        }
        .to_string();

        let sev_lower = entry.severity.trim().to_ascii_lowercase();
        let severity = match sev_lower.as_str() {
            "critical" => "critical",
            "important" => "important",
            "moderate" => "moderate",
            "low" => "low",
            _ => {
                if category == "security" {
                    "important"
                } else {
                    "moderate"
                }
            }
        }
        .to_string();

        let reboot_behavior = match entry.reboot_behavior {
            0 => "never_reboots",
            1 => "always_requires_reboot",
            _ => "may_require_reboot",
        }
        .to_string();

        out.push(PendingOsUpdate {
            update_id: effective_id,
            title: entry.title,
            severity,
            category,
            size_bytes: entry.size_bytes,
            reboot_behavior,
        });
    }

    Ok(out)
}

pub fn build_wua_scan_powershell_script() -> &'static str {
    r#"
$ErrorActionPreference = 'Stop'
$Session = New-Object -ComObject Microsoft.Update.Session
$Searcher = $Session.CreateUpdateSearcher()
$SearchResult = $Searcher.Search("IsInstalled=0 and IsHidden=0")
$Out = @()
foreach ($u in $SearchResult.Updates) {
    $kb = ""
    if ($u.KBArticleIDs.Count -gt 0) { $kb = "KB" + $u.KBArticleIDs.Item(0) }
    $cat = ""
    if ($u.Categories.Count -gt 0) { $cat = $u.Categories.Item(0).Name }
    $sev = [string]$u.MsrcSeverity
    $rb = 2
    try { $rb = [int]$u.InstallationBehavior.RebootBehavior } catch {}
    $Out += [PSCustomObject]@{
        update_id = [string]$u.Identity.UpdateID
        kb = $kb
        title = [string]$u.Title
        severity = $sev
        category = $cat
        size_bytes = [uint64]$u.MaxDownloadSize
        reboot_behavior = $rb
    }
}
if ($Out.Count -eq 0) { Write-Output "[]" } else { $Out | ConvertTo-Json -Compress -Depth 3 }
"#
}

pub fn build_wua_install_powershell_script(
    approved_ids: &[String],
    excluded_updates: &[String],
    security_only: bool,
) -> String {
    let install_all = approved_ids.is_empty()
        || approved_ids
            .iter()
            .any(|id| id.eq_ignore_ascii_case("all") || id == "*");

    let approved_norms: Vec<String> = approved_ids
        .iter()
        .map(|s| format!("'{}'", normalize_kb_id(s).replace('\'', "''")))
        .collect();
    let excluded_norms: Vec<String> = excluded_updates
        .iter()
        .map(|s| format!("'{}'", normalize_kb_id(s).replace('\'', "''")))
        .collect();

    let install_all_ps = if install_all { "$true" } else { "$false" };
    let sec_only_ps = if security_only { "$true" } else { "$false" };

    format!(
        r#"
$ErrorActionPreference = 'Stop'
$InstallAll = {install_all_ps}
$SecurityOnly = {sec_only_ps}
$Approved = @({approved_list})
$Excluded = @({excluded_list})

$Session = New-Object -ComObject Microsoft.Update.Session
$Searcher = $Session.CreateUpdateSearcher()
$SearchResult = $Searcher.Search("IsInstalled=0 and IsHidden=0")
$ToInstall = New-Object -ComObject Microsoft.Update.UpdateColl

foreach ($u in $SearchResult.Updates) {{
    $kb = ""
    if ($u.KBArticleIDs.Count -gt 0) {{ $kb = ("KB" + $u.KBArticleIDs.Item(0)).ToUpper() }}
    $uid = ([string]$u.Identity.UpdateID).ToUpper()
    if (($kb -ne "" -and $Excluded -contains $kb) -or ($Excluded -contains $uid)) {{
        continue
    }}
    if ($SecurityOnly) {{
        $isSec = $false
        foreach ($c in $u.Categories) {{
            if ($c.Name -match 'Security|Critical') {{ $isSec = $true }}
        }}
        if (-not $isSec) {{ continue }}
    }}
    if ($InstallAll -or ($kb -ne "" -and $Approved -contains $kb) -or ($Approved -contains $uid)) {{
        if (-not $u.EulaAccepted) {{ $u.AcceptEula() }}
        [void]$ToInstall.Add($u)
    }}
}}

if ($ToInstall.Count -eq 0) {{
    [PSCustomObject]@{{ result_code = 2; reboot_required = $false; installed_count = 0; message = "No matching updates pending" }} | ConvertTo-Json -Compress
    exit 0
}}

$Downloader = $Session.CreateUpdateDownloader()
$Downloader.Updates = $ToInstall
[void]$Downloader.Download()

$Installer = $Session.CreateUpdateInstaller()
$Installer.Updates = $ToInstall
$Res = $Installer.Install()

[PSCustomObject]@{{
    result_code = [int]$Res.ResultCode
    reboot_required = [bool]$Res.RebootRequired
    installed_count = [int]$ToInstall.Count
    message = "WUA Install ResultCode=" + $Res.ResultCode
}} | ConvertTo-Json -Compress
"#,
        approved_list = approved_norms.join(", "),
        excluded_list = excluded_norms.join(", ")
    )
}

pub trait WuaProvider: Send + Sync {
    fn scan<'a>(
        &'a self,
        excluded_updates: &'a [String],
        timeout: Duration,
    ) -> std::pin::Pin<Box<dyn std::future::Future<Output = Result<Vec<PendingOsUpdate>>> + Send + 'a>>;

    fn install<'a>(
        &'a self,
        approved_ids: &'a [String],
        excluded_updates: &'a [String],
        security_only: bool,
    ) -> std::pin::Pin<
        Box<dyn std::future::Future<Output = Result<PatchExecutionOutcome>> + Send + 'a>,
    >;
}

pub struct PowerShellWuaProvider;

impl WuaProvider for PowerShellWuaProvider {
    fn scan<'a>(
        &'a self,
        excluded_updates: &'a [String],
        timeout: Duration,
    ) -> std::pin::Pin<Box<dyn std::future::Future<Output = Result<Vec<PendingOsUpdate>>> + Send + 'a>>
    {
        Box::pin(async move {
            if let Ok(mock_json) = std::env::var("SWM_MOCK_WUA_JSON") {
                return parse_wua_scan_json(&mock_json, excluded_updates);
            }
            if std::env::var("SWM_SKIP_LIVE_WUA_SCAN").ok().as_deref() == Some("1") {
                return Ok(Vec::new());
            }
            if !cfg!(windows) {
                return Ok(Vec::new());
            }

            let script = build_wua_scan_powershell_script();
            let mut child_cmd = Command::new("powershell.exe");
            child_cmd
                .args([
                    "-NoProfile",
                    "-NonInteractive",
                    "-ExecutionPolicy",
                    "Bypass",
                    "-Command",
                    script,
                ])
                .kill_on_drop(true);

            let output = tokio::time::timeout(timeout, child_cmd.output())
                .await
                .context("Windows Update scan timed out")?
                .context("Failed invoking powershell.exe for WUA scan")?;

            if !output.status.success() {
                let stderr = String::from_utf8_lossy(&output.stderr);
                anyhow::bail!("WUA scan failed: {stderr}");
            }
            let stdout = String::from_utf8_lossy(&output.stdout);
            parse_wua_scan_json(&stdout, excluded_updates)
        })
    }

    fn install<'a>(
        &'a self,
        approved_ids: &'a [String],
        excluded_updates: &'a [String],
        security_only: bool,
    ) -> std::pin::Pin<
        Box<dyn std::future::Future<Output = Result<PatchExecutionOutcome>> + Send + 'a>,
    > {
        Box::pin(async move {
            if let Ok(mock_json) = std::env::var("SWM_MOCK_WUA_INSTALL_JSON") {
                let parsed: RawWuaInstallResult = serde_json::from_str(&mock_json)?;
                let succeeded = matches!(parsed.result_code, 2 | 3);
                return Ok(PatchExecutionOutcome {
                    succeeded,
                    exit_code: if succeeded { 0 } else { parsed.result_code },
                    reboot_required: parsed.reboot_required,
                    installed_count: parsed.installed_count,
                    stdout: parsed.message,
                    stderr: String::new(),
                });
            }

            if !cfg!(windows) {
                anyhow::bail!("Windows WUA installer cannot run on non-Windows platform");
            }

            let script =
                build_wua_install_powershell_script(approved_ids, excluded_updates, security_only);
            let mut child_cmd = Command::new("powershell.exe");
            child_cmd
                .args([
                    "-NoProfile",
                    "-NonInteractive",
                    "-ExecutionPolicy",
                    "Bypass",
                    "-Command",
                    &script,
                ])
                .kill_on_drop(true);

            let output = tokio::time::timeout(Duration::from_secs(3600), child_cmd.output())
                .await
                .context("WUA update installation timed out")?
                .context("Failed invoking powershell.exe for WUA install")?;

            let stdout = String::from_utf8_lossy(&output.stdout).trim().to_string();
            let stderr = String::from_utf8_lossy(&output.stderr).trim().to_string();

            if !output.status.success() {
                return Ok(PatchExecutionOutcome {
                    succeeded: false,
                    exit_code: output.status.code().unwrap_or(1),
                    reboot_required: false,
                    installed_count: 0,
                    stdout,
                    stderr,
                });
            }

            if let Ok(parsed) = serde_json::from_str::<RawWuaInstallResult>(&stdout) {
                // WUA OperationResultCode: 2 = orcSucceeded, 3 = orcSucceededWithErrors
                let succeeded = matches!(parsed.result_code, 2 | 3);
                let exit_code = if succeeded {
                    if parsed.reboot_required {
                        3010
                    } else {
                        0
                    }
                } else {
                    parsed.result_code
                };
                return Ok(PatchExecutionOutcome {
                    succeeded,
                    exit_code,
                    reboot_required: parsed.reboot_required,
                    installed_count: parsed.installed_count,
                    stdout: parsed.message,
                    stderr,
                });
            }

            Ok(PatchExecutionOutcome {
                succeeded: true,
                exit_code: 0,
                reboot_required: false,
                installed_count: 0,
                stdout,
                stderr,
            })
        })
    }
}

pub struct MockWuaProvider {
    pub raw_scan_json: String,
    pub install_reboot_required: bool,
}

impl WuaProvider for MockWuaProvider {
    fn scan<'a>(
        &'a self,
        excluded_updates: &'a [String],
        _timeout: Duration,
    ) -> std::pin::Pin<Box<dyn std::future::Future<Output = Result<Vec<PendingOsUpdate>>> + Send + 'a>>
    {
        Box::pin(async move { parse_wua_scan_json(&self.raw_scan_json, excluded_updates) })
    }

    fn install<'a>(
        &'a self,
        approved_ids: &'a [String],
        excluded_updates: &'a [String],
        _security_only: bool,
    ) -> std::pin::Pin<
        Box<dyn std::future::Future<Output = Result<PatchExecutionOutcome>> + Send + 'a>,
    > {
        Box::pin(async move {
            let candidates = parse_wua_scan_json(&self.raw_scan_json, excluded_updates)?;
            let install_all = approved_ids.is_empty()
                || approved_ids.iter().any(|a| a.eq_ignore_ascii_case("all"));
            let approved_set: HashSet<String> =
                approved_ids.iter().map(|s| normalize_kb_id(s)).collect();

            let matched: Vec<_> = candidates
                .into_iter()
                .filter(|u| install_all || approved_set.contains(&normalize_kb_id(&u.update_id)))
                .collect();

            Ok(PatchExecutionOutcome {
                succeeded: true,
                exit_code: if self.install_reboot_required && !matched.is_empty() {
                    3010
                } else {
                    0
                },
                reboot_required: self.install_reboot_required && !matched.is_empty(),
                installed_count: matched.len(),
                stdout: format!("Installed {} Windows updates", matched.len()),
                stderr: String::new(),
            })
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_windows_wua_scan_and_kb_exclusion_filter() {
        let raw_json = r#"[
            {
                "update_id": "11111111-2222-3333-4444-555555555555",
                "kb": "KB5044285",
                "title": "2024-10 Cumulative Update for Windows 11 (KB5044285)",
                "severity": "Critical",
                "category": "Security Updates",
                "size_bytes": 524288000,
                "reboot_behavior": 1
            },
            {
                "update_id": "99999999-8888-7777-6666-555555555555",
                "kb": "KB5034441",
                "title": "Windows Recovery Environment Update (KB5034441)",
                "severity": "Important",
                "category": "Security Updates",
                "size_bytes": 12000000,
                "reboot_behavior": 2
            }
        ]"#;

        let mock = MockWuaProvider {
            raw_scan_json: raw_json.to_string(),
            install_reboot_required: true,
        };

        // Exclude 5034441 (even without KB prefix, normalize_kb_id handles it!)
        let excluded = vec!["5034441".to_string()];
        let pending = mock.scan(&excluded, Duration::from_secs(5)).await.unwrap();
        assert_eq!(pending.len(), 1);
        assert_eq!(pending[0].update_id, "KB5044285");
        assert_eq!(pending[0].severity, "critical");
        assert_eq!(pending[0].category, "security");
        assert_eq!(pending[0].reboot_behavior, "always_requires_reboot");

        // Install approved update -> maps reboot_required & exit code 3010
        let outcome = mock
            .install(&["KB5044285".to_string()], &excluded, false)
            .await
            .unwrap();
        assert!(outcome.succeeded);
        assert!(outcome.reboot_required);
        assert_eq!(outcome.exit_code, 3010);
        assert_eq!(outcome.installed_count, 1);
    }

    #[test]
    fn test_windows_wua_single_object_json_and_script_generation() {
        let single_obj = r#"{
            "update_id": "abc-123",
            "kb": "KB5001234",
            "title": "Defender Security Intelligence Update",
            "severity": "",
            "category": "Definition Updates",
            "size_bytes": 4096,
            "reboot_behavior": 0
        }"#;
        let parsed = parse_wua_scan_json(single_obj, &[]).unwrap();
        assert_eq!(parsed.len(), 1);
        assert_eq!(parsed[0].reboot_behavior, "never_reboots");

        let ps = build_wua_install_powershell_script(
            &["KB5044285".to_string()],
            &["KB5034441".to_string()],
            true,
        );
        assert!(ps.contains("$Approved = @('KB5044285')"));
        assert!(ps.contains("$Excluded = @('KB5034441')"));
        assert!(ps.contains("$SecurityOnly = $true"));
    }
}
