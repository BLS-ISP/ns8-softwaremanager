use crate::dto::{AdminDesktopMessage, DesktopPolicy, PendingOsUpdate, RebootState};
use serde::{Deserialize, Serialize};

pub const DEFAULT_WINDOWS_PIPE: &str = r"\\.\pipe\ns8-swm-ipc";
pub const DEFAULT_MACOS_SOCKET: &str = "/var/run/ns8-swm/ipc.sock";
pub const DEFAULT_UNIX_SOCKET: &str = "/run/ns8-swm/ipc.sock";
pub const MAX_IPC_FRAME_BYTES: usize = 1024 * 1024; // 1 MiB strict cap

pub fn default_ipc_address_for_os(os_family: &str) -> &'static str {
    match os_family.trim().to_ascii_lowercase().as_str() {
        "windows" => DEFAULT_WINDOWS_PIPE,
        "macos" | "darwin" => DEFAULT_MACOS_SOCKET,
        _ => DEFAULT_UNIX_SOCKET,
    }
}

pub fn default_ipc_address() -> &'static str {
    if cfg!(windows) {
        DEFAULT_WINDOWS_PIPE
    } else if cfg!(target_os = "macos") {
        DEFAULT_MACOS_SOCKET
    } else {
        DEFAULT_UNIX_SOCKET
    }
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum PackageAction {
    Install,
    Update,
    Uninstall,
}

impl PackageAction {
    pub fn as_wire(&self) -> &'static str {
        match self {
            Self::Install => "install",
            Self::Update => "update",
            Self::Uninstall => "uninstall",
        }
    }
}

/// Strictly typed unprivileged-to-privileged local IPC request enum.
/// By design, callers CANNOT supply raw command strings, paths, or download URLs;
/// only `package_id`s from the Ed25519-verified `available_catalog` are accepted.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(tag = "command", rename_all = "snake_case")]
pub enum IpcRequest {
    ListCatalog,
    GetStatus,
    TriggerPackage {
        package_id: String,
        action: PackageAction,
    },
    TriggerPendingUpdates,
    SnoozeReboot {
        minutes: u32,
    },
    ConfirmRebootNow,
    SyncNow,
    DismissMessage {
        message_id: String,
    },
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum IpcErrorCode {
    PermissionDenied,
    InvalidRequest,
    NotFound,
    DeadlineExceeded,
    AgentOffline,
    InternalError,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct CatalogViewItem {
    pub package_id: String,
    pub name: String,
    pub vendor: String,
    pub version: String,
    pub description: String,
    pub category: String,
    pub icon_url: String,
    pub os_family: String,
    pub source_type: String,
    pub allow_uninstall: bool,
    pub installed_version: Option<String>,
    /// `"available"` | `"downloading"` | `"installing"` | `"waiting_for_process"` | `"installed"` | `"update_available"` | `"reboot_required"` | `"failed"`
    pub status: String,
    pub blocked_processes: Vec<String>,
    pub last_message: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct JobActivityEntry {
    pub job_id: String,
    pub job_type: String,
    pub action: String,
    pub target_name: String,
    pub version: String,
    pub status: String,
    pub exit_code: Option<i32>,
    pub reboot_required: bool,
    pub timestamp: String,
    pub message: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct CenterStatusSnapshot {
    pub device_id: String,
    #[serde(default)]
    pub hostname: String,
    pub server_url: String,
    pub agent_version: String,
    pub connected: bool,
    pub device_status: String,
    pub pending_os_updates: Vec<PendingOsUpdate>,
    pub active_jobs: Vec<JobActivityEntry>,
    pub recent_history: Vec<JobActivityEntry>,
    pub reboot_state: RebootState,
    #[serde(default)]
    pub desktop_policy: DesktopPolicy,
    #[serde(default)]
    pub admin_messages: Vec<AdminDesktopMessage>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum IpcResponseData {
    Catalog {
        items: Vec<CatalogViewItem>,
    },
    Status(CenterStatusSnapshot),
    ActionTriggered {
        job_id: String,
        status: String,
        message: String,
    },
    RebootUpdated(RebootState),
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct IpcResponse {
    pub ok: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error_code: Option<IpcErrorCode>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub message: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub data: Option<IpcResponseData>,
}

impl IpcResponse {
    pub fn success(data: IpcResponseData) -> Self {
        Self {
            ok: true,
            error_code: None,
            message: None,
            data: Some(data),
        }
    }

    pub fn error(code: IpcErrorCode, message: impl Into<String>) -> Self {
        Self {
            ok: false,
            error_code: Some(code),
            message: Some(message.into()),
            data: None,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_ipc_request_serialization_roundtrip() {
        let req = IpcRequest::TriggerPackage {
            package_id: "pkg-123".to_string(),
            action: PackageAction::Install,
        };
        let json = serde_json::to_string(&req).unwrap();
        assert!(json.contains("\"command\":\"trigger_package\""));
        let decoded: IpcRequest = serde_json::from_str(&json).unwrap();
        assert_eq!(decoded, req);
    }

    #[test]
    fn test_ipc_rejects_unknown_arbitrary_command_variant() {
        let raw = r#"{"command":"run_shell","cmd":"whoami"}"#;
        assert!(serde_json::from_str::<IpcRequest>(raw).is_err());
    }

    #[test]
    fn test_desktop_policy_defaults_and_new_ipc_variants_roundtrip() {
        let def = DesktopPolicy::default();
        assert!(def.enforce_tray);
        assert!(def.allow_self_service);
        assert!(def.allow_user_os_updates);
        assert!(!def.silent_notifications);

        let from_empty: DesktopPolicy = serde_json::from_str("{}").unwrap();
        assert_eq!(from_empty, def);

        let sync_json = serde_json::to_string(&IpcRequest::SyncNow).unwrap();
        assert!(sync_json.contains("\"command\":\"sync_now\""));
        assert_eq!(
            serde_json::from_str::<IpcRequest>(&sync_json).unwrap(),
            IpcRequest::SyncNow
        );

        let dismiss_req = IpcRequest::DismissMessage {
            message_id: "msg-42".to_string(),
        };
        let dismiss_json = serde_json::to_string(&dismiss_req).unwrap();
        assert!(dismiss_json.contains("\"command\":\"dismiss_message\""));
        assert_eq!(
            serde_json::from_str::<IpcRequest>(&dismiss_json).unwrap(),
            dismiss_req
        );
    }

    #[test]
    fn test_macos_ipc_socket_and_detection_rule_wire_contract() {
        assert_eq!(DEFAULT_MACOS_SOCKET, "/var/run/ns8-swm/ipc.sock");
        assert_eq!(DEFAULT_UNIX_SOCKET, "/run/ns8-swm/ipc.sock");
        assert_eq!(default_ipc_address_for_os("macos"), DEFAULT_MACOS_SOCKET);
        assert_eq!(default_ipc_address_for_os("linux"), DEFAULT_UNIX_SOCKET);
        assert_eq!(default_ipc_address_for_os("windows"), DEFAULT_WINDOWS_PIPE);
        assert_eq!(crate::dto::AGENT_VERSION, "1.2.0");

        let app_rule: crate::dto::DetectionRule = serde_json::from_str(
            r#"{"type":"app_bundle","path":"/Applications/Firefox.app","bundle_id":"org.mozilla.firefox","min_version":"132.0"}"#,
        )
        .unwrap();
        assert_eq!(app_rule.rule_type, "app_bundle");
        assert_eq!(app_rule.path.as_deref(), Some("/Applications/Firefox.app"));
        assert_eq!(app_rule.bundle_id.as_deref(), Some("org.mozilla.firefox"));
        assert_eq!(app_rule.effective_version(), Some("132.0"));

        let brew_rule: crate::dto::DetectionRule = serde_json::from_str(
            r#"{"type":"brew_cask","package":"firefox","version":null,"min_version":"131.0","package_name":null}"#,
        )
        .unwrap();
        assert_eq!(brew_rule.rule_type, "brew_cask");
        assert_eq!(brew_rule.effective_package(), Some("firefox"));
        assert_eq!(brew_rule.effective_version(), Some("131.0"));
    }
}
