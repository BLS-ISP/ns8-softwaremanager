use serde::{Deserialize, Deserializer, Serialize};
use std::collections::HashMap;

pub const AGENT_VERSION: &str = env!("CARGO_PKG_VERSION");

/// Deserializes an ID field that might be a JSON string, integer, or null (defaulting to `""`).
pub fn deserialize_id<'de, D>(deserializer: D) -> Result<String, D::Error>
where
    D: Deserializer<'de>,
{
    let val = Option::<serde_json::Value>::deserialize(deserializer)?;
    match val {
        None | Some(serde_json::Value::Null) => Ok(String::new()),
        Some(serde_json::Value::String(s)) => Ok(s),
        Some(serde_json::Value::Number(n)) => Ok(n.to_string()),
        Some(other) => Ok(other.to_string()),
    }
}

/// Deserializes `Option<String>` treating `null` as `None`.
pub fn deserialize_opt_string<'de, D>(deserializer: D) -> Result<Option<String>, D::Error>
where
    D: Deserializer<'de>,
{
    let val = Option::<serde_json::Value>::deserialize(deserializer)?;
    match val {
        None | Some(serde_json::Value::Null) => Ok(None),
        Some(serde_json::Value::String(s)) => Ok(Some(s)),
        Some(serde_json::Value::Number(n)) => Ok(Some(n.to_string())),
        Some(other) => Ok(Some(other.to_string())),
    }
}

/// Deserializes `HashMap<String, String>` even if values are numbers or strings, or if null.
pub fn deserialize_return_codes<'de, D>(
    deserializer: D,
) -> Result<HashMap<String, String>, D::Error>
where
    D: Deserializer<'de>,
{
    let val = Option::<HashMap<String, serde_json::Value>>::deserialize(deserializer)?;
    let mut out = HashMap::new();
    if let Some(map) = val {
        for (k, v) in map {
            let mapped = match v {
                serde_json::Value::String(s) => s,
                serde_json::Value::Number(n) => n.to_string(),
                other => other.to_string(),
            };
            out.insert(k, mapped);
        }
    }
    Ok(out)
}

fn default_reboot_policy_str() -> String {
    "never".to_string()
}

fn default_grace_period_minutes() -> u32 {
    240
}

fn default_reboot_behavior() -> String {
    "may_require_reboot".to_string()
}

fn default_scope() -> String {
    "machine".to_string()
}

fn default_true() -> bool {
    true
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct DesktopPolicy {
    #[serde(default = "default_true")]
    pub enforce_tray: bool,
    #[serde(default = "default_true")]
    pub allow_self_service: bool,
    #[serde(default = "default_true")]
    pub allow_user_os_updates: bool,
    #[serde(default)]
    pub silent_notifications: bool,
}

impl Default for DesktopPolicy {
    fn default() -> Self {
        Self {
            enforce_tray: true,
            allow_self_service: true,
            allow_user_os_updates: true,
            silent_notifications: false,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct AdminDesktopMessage {
    #[serde(default, deserialize_with = "deserialize_id")]
    pub id: String,
    #[serde(default)]
    pub title: String,
    #[serde(default)]
    pub message: String,
    #[serde(default)]
    pub urgent: bool,
    #[serde(default)]
    pub created_at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct EnrollRequest {
    pub enrollment_token: String,
    pub hostname: String,
    pub os_family: String,
    pub os_name: String,
    pub os_version: String,
    pub arch: String,
    pub agent_version: String,
    pub machine_uid: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct EnrollResponse {
    #[serde(deserialize_with = "deserialize_id")]
    pub device_id: String,
    pub device_token: String,
    pub status: String,
    pub server_public_key_b64: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct AgentStatusResponse {
    #[serde(default, deserialize_with = "deserialize_id")]
    pub device_id: String,
    pub status: String,
    #[serde(default = "default_reboot_policy_str")]
    pub reboot_policy: String,
    #[serde(default = "default_grace_period_minutes")]
    pub grace_period_minutes: u32,
    #[serde(default)]
    pub in_maintenance_window: bool,
    #[serde(default)]
    pub excluded_updates: Vec<String>,
    #[serde(default, deserialize_with = "deserialize_opt_string")]
    pub server_public_key_b64: Option<String>,
    #[serde(default)]
    pub desktop_policy: DesktopPolicy,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct DiskInfo {
    pub mount: String,
    pub total_gb: f64,
    pub free_gb: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Default)]
pub struct HardwareInfo {
    pub cpu_model: String,
    pub cpu_cores: u32,
    pub ram_total_mb: u64,
    pub ram_free_mb: u64,
    #[serde(default)]
    pub disks: Vec<DiskInfo>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct SoftwareEntry {
    pub name: String,
    pub version: String,
    #[serde(default, alias = "vendor")]
    pub publisher: String,
    pub source: String,
    #[serde(default = "default_scope")]
    pub scope: String,
    #[serde(default)]
    pub package_ref: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct PendingOsUpdate {
    pub update_id: String,
    pub title: String,
    pub severity: String,
    pub category: String,
    #[serde(default)]
    pub size_bytes: u64,
    #[serde(default = "default_reboot_behavior")]
    pub reboot_behavior: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct InventoryReport {
    pub hostname: String,
    #[serde(default)]
    pub domain: String,
    pub os_family: String,
    pub os_name: String,
    pub os_version: String,
    pub kernel_or_build: String,
    pub arch: String,
    pub agent_version: String,
    pub hardware: HardwareInfo,
    #[serde(default)]
    pub ip_addresses: Vec<String>,
    #[serde(default)]
    pub logged_in_users: Vec<String>,
    pub reboot_required: bool,
    #[serde(default)]
    pub software_hash: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub installed_software: Option<Vec<SoftwareEntry>>,
    #[serde(default)]
    pub pending_os_updates: Vec<PendingOsUpdate>,
    #[serde(default)]
    pub center_running: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct InventoryAckResponse {
    #[serde(default)]
    pub status: String,
    #[serde(default)]
    pub enqueued_jobs: u32,
    #[serde(default)]
    pub software_updated: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Default)]
pub struct DetectionRule {
    #[serde(default, rename = "type", alias = "rule_type")]
    pub rule_type: String,
    #[serde(default)]
    pub path: Option<String>,
    #[serde(default, alias = "key")]
    pub registry_key: Option<String>,
    #[serde(default, alias = "value")]
    pub value_name: Option<String>,
    #[serde(default, alias = "package_id", alias = "package_ref")]
    pub package_name: Option<String>,
    #[serde(default, alias = "expected_version")]
    pub version: Option<String>,
    #[serde(default, alias = "version_operator")]
    pub operator: Option<String>,
}

impl DetectionRule {
    pub fn is_empty(&self) -> bool {
        self.rule_type.trim().is_empty()
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Default)]
pub struct PackageMeta {
    #[serde(default, deserialize_with = "deserialize_id")]
    pub package_id: String,
    #[serde(default)]
    pub name: String,
    #[serde(default)]
    pub vendor: String,
    #[serde(default)]
    pub os_family: String,
    #[serde(default)]
    pub source_type: String,
    #[serde(default)]
    pub package_ref: String,
    #[serde(default)]
    pub filename: String,
    #[serde(default)]
    pub file_size: u64,
    #[serde(default)]
    pub install_args: String,
    #[serde(default)]
    pub uninstall_args: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct SignedJob {
    #[serde(deserialize_with = "deserialize_id")]
    pub job_id: String,
    #[serde(deserialize_with = "deserialize_id")]
    pub device_id: String,
    pub job_type: String,
    pub action: String,
    #[serde(default, deserialize_with = "deserialize_id")]
    pub target_id: String,
    #[serde(default)]
    pub version: String,
    #[serde(default, deserialize_with = "deserialize_opt_string")]
    pub sha256: Option<String>,
    #[serde(default, deserialize_with = "deserialize_opt_string")]
    pub command: Option<String>,
    pub expires_at_unix: i64,
    pub signature_b64: String,

    // Extended execution metadata (covered indirectly via package_id/target_id/sha256/command/version)
    #[serde(default, deserialize_with = "deserialize_opt_string")]
    pub download_url: Option<String>,
    #[serde(default)]
    pub update_ids: Vec<String>,
    #[serde(default)]
    pub security_only: bool,
    #[serde(default)]
    pub package_meta: Option<PackageMeta>,
    #[serde(default)]
    pub detection_rule: Option<DetectionRule>,
    #[serde(default)]
    pub blocked_processes: Vec<String>,
    #[serde(default, deserialize_with = "deserialize_return_codes")]
    pub return_codes: HashMap<String, String>,
}

impl SignedJob {
    /// Produces the exact canonical UTF-8 payload string signed by the server:
    /// `v1|job|{job_id}|{device_id}|{job_type}|{action}|{target_id}|{version}|{sha256_or_empty}|{command_or_empty}|{expires_at_unix}`
    pub fn canonical_string(&self) -> String {
        format!(
            "v1|job|{}|{}|{}|{}|{}|{}|{}|{}|{}",
            self.job_id,
            self.device_id,
            self.job_type,
            self.action,
            self.target_id,
            self.version,
            self.sha256.as_deref().unwrap_or(""),
            self.command.as_deref().unwrap_or(""),
            self.expires_at_unix
        )
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct SignedCatalogItem {
    #[serde(deserialize_with = "deserialize_id")]
    pub package_id: String,
    #[serde(deserialize_with = "deserialize_id")]
    pub device_id: String,
    #[serde(default)]
    pub name: String,
    #[serde(default)]
    pub vendor: String,
    #[serde(default)]
    pub description: String,
    #[serde(default)]
    pub category: String,
    #[serde(default)]
    pub icon_url: String,
    #[serde(default)]
    pub os_family: String,
    pub source_type: String,
    #[serde(default)]
    pub package_ref: String,
    #[serde(default)]
    pub version: String,
    #[serde(default, deserialize_with = "deserialize_opt_string")]
    pub sha256: Option<String>,
    #[serde(default)]
    pub file_size: u64,
    #[serde(default, deserialize_with = "deserialize_opt_string")]
    pub download_url: Option<String>,
    #[serde(default, alias = "install_args", deserialize_with = "deserialize_opt_string")]
    pub install_cmd: Option<String>,
    #[serde(default, alias = "uninstall_args", deserialize_with = "deserialize_opt_string")]
    pub uninstall_cmd: Option<String>,
    #[serde(default)]
    pub allow_uninstall: bool,
    #[serde(default, deserialize_with = "deserialize_opt_string")]
    pub installed_version: Option<String>,
    #[serde(default)]
    pub status: String,
    #[serde(default)]
    pub blocked_processes: Vec<String>,
    #[serde(default)]
    pub detection_rule: Option<DetectionRule>,
    #[serde(default, deserialize_with = "deserialize_return_codes")]
    pub return_codes: HashMap<String, String>,
    pub signature_b64: String,
}

impl SignedCatalogItem {
    /// Produces the exact canonical UTF-8 payload string signed by the server:
    /// `v1|catalog|{package_id}|{device_id}|{source_type}|{package_ref}|{version}|{sha256_or_empty}|{install_cmd_or_empty}|{uninstall_cmd_or_empty}|{allow_uninstall_0_or_1}`
    pub fn canonical_string(&self) -> String {
        format!(
            "v1|catalog|{}|{}|{}|{}|{}|{}|{}|{}|{}",
            self.package_id,
            self.device_id,
            self.source_type,
            self.package_ref,
            self.version,
            self.sha256.as_deref().unwrap_or(""),
            self.install_cmd.as_deref().unwrap_or(""),
            self.uninstall_cmd.as_deref().unwrap_or(""),
            if self.allow_uninstall { "1" } else { "0" }
        )
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct PendingJobsResponse {
    #[serde(default)]
    pub jobs: Vec<SignedJob>,
    #[serde(default)]
    pub available_catalog: Vec<SignedCatalogItem>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct JobResult {
    pub status: String,
    pub exit_code: i32,
    pub reboot_required: bool,
    pub stdout: String,
    pub stderr: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct SelfServiceTriggerRequest {
    pub package_id: String,
    pub action: String,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum RebootPolicy {
    Never,
    MaintenanceWindow,
    GracePeriod,
}

impl Default for RebootPolicy {
    fn default() -> Self {
        Self::Never
    }
}

impl RebootPolicy {
    pub fn from_wire(s: &str) -> Self {
        match s.trim().to_ascii_lowercase().as_str() {
            "maintenance_window" => Self::MaintenanceWindow,
            "grace_period" => Self::GracePeriod,
            _ => Self::Never,
        }
    }

    pub fn as_wire(&self) -> &'static str {
        match self {
            Self::Never => "never",
            Self::MaintenanceWindow => "maintenance_window",
            Self::GracePeriod => "grace_period",
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct RebootState {
    pub reboot_required: bool,
    pub policy: RebootPolicy,
    pub grace_period_minutes: u32,
    pub detected_at_unix: Option<i64>,
    pub deadline_unix: Option<i64>,
    pub snoozed_until_unix: Option<i64>,
    pub triggered_by_job_id: Option<String>,
}

impl Default for RebootState {
    fn default() -> Self {
        Self {
            reboot_required: false,
            policy: RebootPolicy::Never,
            grace_period_minutes: 240,
            detected_at_unix: None,
            deadline_unix: None,
            snoozed_until_unix: None,
            triggered_by_job_id: None,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum WsAgentFrame {
    Heartbeat {
        agent_version: String,
        reboot_required: bool,
    },
    LogChunk {
        job_id: String,
        stream: String,
        chunk: String,
    },
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct WsServerFrame {
    #[serde(rename = "type")]
    pub frame_type: String,
    #[serde(default)]
    pub job_id: Option<String>,
    #[serde(default)]
    pub server_time: Option<String>,
}
