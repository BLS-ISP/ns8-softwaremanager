use anyhow::{Context, Result};
use chrono::Utc;
use serde::{Deserialize, Serialize};
use std::fs;
use std::path::{Path, PathBuf};
use swm_common::{
    AdminDesktopMessage, DesktopPolicy, JobActivityEntry, PendingOsUpdate, RebootState,
    SignedCatalogItem, SoftwareEntry,
};

pub const DEFAULT_WINDOWS_DATA_DIR: &str = r"C:\ProgramData\NS8-SoftwareManager";
pub const DEFAULT_MACOS_DATA_DIR: &str = "/Library/Application Support/NS8-SoftwareManager";
pub const DEFAULT_LINUX_DATA_DIR: &str = "/etc/ns8-swm";

pub fn default_data_dir() -> PathBuf {
    if cfg!(windows) {
        PathBuf::from(DEFAULT_WINDOWS_DATA_DIR)
    } else if cfg!(target_os = "macos") {
        PathBuf::from(DEFAULT_MACOS_DATA_DIR)
    } else {
        PathBuf::from(DEFAULT_LINUX_DATA_DIR)
    }
}

pub fn normalize_server_url(raw: &str) -> String {
    raw.trim().trim_end_matches('/').to_string()
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct AgentConfig {
    pub server_url: String,
    pub device_id: String,
    pub device_token: String,
    pub status: String,
    pub server_public_key_b64: String,
    pub enrolled_at: String,
}

impl AgentConfig {
    pub fn config_path(data_dir: &Path) -> PathBuf {
        data_dir.join("config.json")
    }

    pub fn load(data_dir: &Path) -> Result<Self> {
        let path = Self::config_path(data_dir);
        let content = fs::read_to_string(&path)
            .with_context(|| format!("Agent is not enrolled (missing {})", path.display()))?;
        let mut cfg: Self = serde_json::from_str(&content)
            .with_context(|| format!("Failed parsing {}", path.display()))?;
        cfg.server_url = normalize_server_url(&cfg.server_url);
        Ok(cfg)
    }

    pub fn save(&self, data_dir: &Path) -> Result<()> {
        fs::create_dir_all(data_dir)
            .with_context(|| format!("Failed creating data directory {}", data_dir.display()))?;
        let path = Self::config_path(data_dir);
        atomic_write_json(&path, self, true)
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct AgentRuntimeState {
    #[serde(default)]
    pub last_software_hash: Option<String>,
    #[serde(default)]
    pub reboot_state: RebootState,
    #[serde(default)]
    pub excluded_updates: Vec<String>,
    #[serde(default)]
    pub in_maintenance_window: bool,
    #[serde(default)]
    pub available_catalog: Vec<SignedCatalogItem>,
    #[serde(default)]
    pub installed_software_cache: Vec<SoftwareEntry>,
    #[serde(default)]
    pub pending_os_updates_cache: Vec<PendingOsUpdate>,
    #[serde(default)]
    pub recent_history: Vec<JobActivityEntry>,
    #[serde(default)]
    pub desktop_policy: DesktopPolicy,
    #[serde(default)]
    pub admin_messages: Vec<AdminDesktopMessage>,
}

impl AgentRuntimeState {
    pub fn state_path(data_dir: &Path) -> PathBuf {
        data_dir.join("state.json")
    }

    pub fn staging_dir(data_dir: &Path) -> PathBuf {
        data_dir.join("staging")
    }

    pub fn load_or_default(data_dir: &Path) -> Self {
        let path = Self::state_path(data_dir);
        if let Ok(content) = fs::read_to_string(&path) {
            if let Ok(state) = serde_json::from_str::<Self>(&content) {
                return state;
            }
        }
        Self::default()
    }

    pub fn save(&self, data_dir: &Path) -> Result<()> {
        fs::create_dir_all(data_dir)
            .with_context(|| format!("Failed creating data directory {}", data_dir.display()))?;
        let path = Self::state_path(data_dir);
        atomic_write_json(&path, self, false)
    }

    pub fn record_activity(&mut self, entry: JobActivityEntry) {
        self.recent_history.insert(0, entry);
        if self.recent_history.len() > 100 {
            self.recent_history.truncate(100);
        }
    }

    pub fn now_iso8601() -> String {
        Utc::now().to_rfc3339()
    }
}

pub fn atomic_write_json<T: Serialize>(path: &Path, value: &T, secret: bool) -> Result<()> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }
    let tmp_path = path.with_extension(format!("json.tmp.{}", std::process::id()));
    let serialized = serde_json::to_vec_pretty(value)?;
    fs::write(&tmp_path, serialized)
        .with_context(|| format!("Failed writing temporary file {}", tmp_path.display()))?;

    #[cfg(unix)]
    if secret {
        use std::os::unix::fs::PermissionsExt;
        let _ = fs::set_permissions(&tmp_path, fs::Permissions::from_mode(0o600));
    }
    #[cfg(not(unix))]
    let _ = secret;

    if path.exists() && cfg!(windows) {
        let _ = fs::remove_file(path);
    }
    fs::rename(&tmp_path, path).with_context(|| {
        format!(
            "Failed atomically replacing {} with {}",
            path.display(),
            tmp_path.display()
        )
    })?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    #[test]
    fn test_config_and_state_atomic_persistence() {
        let dir = tempdir().unwrap();
        let cfg = AgentConfig {
            server_url: "https://swm.example.org/".to_string(),
            device_id: "dev-01".to_string(),
            device_token: "tok-secret".to_string(),
            status: "approved".to_string(),
            server_public_key_b64: "AAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAA=".to_string(),
            enrolled_at: AgentRuntimeState::now_iso8601(),
        };
        cfg.save(dir.path()).unwrap();

        let loaded = AgentConfig::load(dir.path()).unwrap();
        assert_eq!(loaded.server_url, "https://swm.example.org");
        assert_eq!(loaded.device_id, "dev-01");
        assert_eq!(loaded.device_token, "tok-secret");
    }
}
