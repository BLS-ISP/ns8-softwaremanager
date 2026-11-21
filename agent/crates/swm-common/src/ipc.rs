use crate::dto::{PendingOsUpdate, RebootState};
use serde::{Deserialize, Serialize};

pub const DEFAULT_WINDOWS_PIPE: &str = r"\\.\pipe\ns8-swm-ipc";
pub const DEFAULT_UNIX_SOCKET: &str = "/run/ns8-swm/ipc.sock";
pub const MAX_IPC_FRAME_BYTES: usize = 1024 * 1024; // 1 MiB strict cap

pub fn default_ipc_address() -> &'static str {
    if cfg!(windows) {
        DEFAULT_WINDOWS_PIPE
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
    pub server_url: String,
    pub agent_version: String,
    pub connected: bool,
    pub device_status: String,
    pub pending_os_updates: Vec<PendingOsUpdate>,
    pub active_jobs: Vec<JobActivityEntry>,
    pub recent_history: Vec<JobActivityEntry>,
    pub reboot_state: RebootState,
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
}
