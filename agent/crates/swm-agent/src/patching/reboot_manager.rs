use crate::config::atomic_write_json;
use anyhow::Result;
use std::fs;
use std::path::{Path, PathBuf};
use swm_common::{RebootPolicy, RebootState};
use thiserror::Error;

#[derive(Debug, Error, PartialEq, Eq)]
pub enum RebootPolicyError {
    #[error("No reboot is currently pending")]
    NoRebootPending,

    #[error("Cannot snooze: grace period deadline has already passed")]
    DeadlineExceeded,

    #[error("Snooze is only supported under grace_period reboot policy")]
    PolicyDoesNotAllowSnooze,
}

/// Manages persistent reboot state & grace-period deadlines across restarts.
/// Strictly enforces `Never`, `MaintenanceWindow`, and `GracePeriod` policies.
#[derive(Debug, Clone)]
pub struct RebootManager {
    state_file: PathBuf,
    state: RebootState,
}

impl RebootManager {
    pub fn load(data_dir: &Path) -> Self {
        let state_file = data_dir.join("reboot_state.json");
        let state = fs::read_to_string(&state_file)
            .ok()
            .and_then(|s| serde_json::from_str::<RebootState>(&s).ok())
            .unwrap_or_default();
        Self { state_file, state }
    }

    pub fn state(&self) -> &RebootState {
        &self.state
    }

    pub fn save(&self) -> Result<()> {
        atomic_write_json(&self.state_file, &self.state, false)
    }

    /// Synchronizes group reboot policy & OS pending reboot state without resetting an active deadline!
    pub fn sync_policy_and_os_state(
        &mut self,
        policy: RebootPolicy,
        grace_period_minutes: u32,
        os_reboot_pending: bool,
        now_unix: i64,
    ) -> Result<()> {
        self.state.policy = policy;
        self.state.grace_period_minutes = grace_period_minutes.max(1);

        if os_reboot_pending && !self.state.reboot_required {
            self.mark_reboot_required(None, now_unix)?;
            return Ok(());
        }

        // If the OS has actually rebooted (no OS indicators left) and a previous reboot was cleared
        if !os_reboot_pending && self.state.reboot_required && self.state.triggered_by_job_id.is_none()
        {
            self.clear_reboot_state()?;
            return Ok(());
        }

        // Ensure deadline exists if policy switched to GracePeriod while reboot is already pending
        if self.state.reboot_required && self.state.policy == RebootPolicy::GracePeriod {
            if self.state.deadline_unix.is_none() {
                let base = self.state.detected_at_unix.unwrap_or(now_unix);
                self.state.detected_at_unix = Some(base);
                self.state.deadline_unix =
                    Some(base + i64::from(self.state.grace_period_minutes) * 60);
            }
        } else if self.state.policy != RebootPolicy::GracePeriod {
            self.state.deadline_unix = None;
            self.state.snoozed_until_unix = None;
        }

        self.save()
    }

    /// Marks the system as requiring a reboot (e.g. after WUA / MSI exit code 3010 / kernel patch).
    /// Preserves any existing `deadline_unix` so repeated updates or service restarts never push back the hard deadline.
    pub fn mark_reboot_required(
        &mut self,
        job_id: Option<String>,
        now_unix: i64,
    ) -> Result<&RebootState> {
        self.state.reboot_required = true;
        if self.state.detected_at_unix.is_none() {
            self.state.detected_at_unix = Some(now_unix);
        }
        if job_id.is_some() {
            self.state.triggered_by_job_id = job_id;
        }
        if self.state.policy == RebootPolicy::GracePeriod && self.state.deadline_unix.is_none() {
            let grace_secs = i64::from(self.state.grace_period_minutes.max(1)) * 60;
            self.state.deadline_unix = Some(now_unix + grace_secs);
        }
        self.save()?;
        Ok(&self.state)
    }

    pub fn clear_reboot_state(&mut self) -> Result<()> {
        self.state.reboot_required = false;
        self.state.detected_at_unix = None;
        self.state.deadline_unix = None;
        self.state.snoozed_until_unix = None;
        self.state.triggered_by_job_id = None;
        self.save()
    }

    /// Snoozes user notifications for `minutes`, clamped strictly to `deadline_unix`.
    pub fn snooze(
        &mut self,
        minutes: u32,
        now_unix: i64,
    ) -> std::result::Result<RebootState, RebootPolicyError> {
        if !self.state.reboot_required {
            return Err(RebootPolicyError::NoRebootPending);
        }
        if self.state.policy != RebootPolicy::GracePeriod {
            return Err(RebootPolicyError::PolicyDoesNotAllowSnooze);
        }
        let deadline = self
            .state
            .deadline_unix
            .unwrap_or(now_unix + i64::from(self.state.grace_period_minutes) * 60);

        if now_unix >= deadline {
            return Err(RebootPolicyError::DeadlineExceeded);
        }

        let requested = now_unix + i64::from(minutes.max(1)) * 60;
        let clamped = requested.min(deadline);
        self.state.snoozed_until_unix = Some(clamped);
        let _ = self.save();
        Ok(self.state.clone())
    }

    /// Evaluates whether the agent is permitted and required to automatically reboot right now.
    pub fn should_execute_auto_reboot(&self, now_unix: i64, in_maintenance_window: bool) -> bool {
        if !self.state.reboot_required {
            return false;
        }
        match self.state.policy {
            // CRITICAL RULE: Servers with `Never` policy NEVER reboot automatically!
            RebootPolicy::Never => false,
            RebootPolicy::MaintenanceWindow => in_maintenance_window,
            RebootPolicy::GracePeriod => self
                .state
                .deadline_unix
                .map(|deadline| now_unix >= deadline)
                .unwrap_or(false),
        }
    }
}

/// Triggers an OS reboot only when explicitly enabled (safe dry-run marker in test environments).
pub fn execute_system_reboot(data_dir: &Path, reason: &str) -> Result<()> {
    let marker = data_dir.join("last_reboot_triggered.txt");
    let _ = fs::write(&marker, format!("reason={reason}\n"));

    // Safety guard: only invoke real OS shutdown when SWM_ENABLE_REAL_REBOOT=1 is set in production service
    if std::env::var("SWM_ENABLE_REAL_REBOOT").ok().as_deref() != Some("1") {
        tracing::info!(
            "Reboot requested ({reason}) - recorded at {} (set SWM_ENABLE_REAL_REBOOT=1 for live OS reboot)",
            marker.display()
        );
        return Ok(());
    }

    #[cfg(windows)]
    {
        std::process::Command::new("shutdown.exe")
            .args([
                "/r",
                "/t",
                "10",
                "/c",
                "NS8-SoftwareManager scheduled system restart",
            ])
            .status()?;
    }

    #[cfg(target_os = "macos")]
    {
        std::process::Command::new("/sbin/shutdown")
            .args(["-r", "now"])
            .status()?;
    }

    #[cfg(all(unix, not(target_os = "macos")))]
    {
        std::process::Command::new("systemctl")
            .arg("reboot")
            .status()?;
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    #[test]
    fn test_reboot_policy_never_prevents_automatic_reboot() {
        let dir = tempdir().unwrap();
        let mut mgr = RebootManager::load(dir.path());
        mgr.sync_policy_and_os_state(RebootPolicy::Never, 240, false, 1000)
            .unwrap();
        mgr.mark_reboot_required(Some("job-1".to_string()), 1000)
            .unwrap();

        assert!(mgr.state().reboot_required);
        assert_eq!(mgr.state().deadline_unix, None);
        assert!(!mgr.should_execute_auto_reboot(1000, false));
        assert!(!mgr.should_execute_auto_reboot(999_999_999, true));
    }

    #[test]
    fn test_reboot_policy_maintenance_window_gating() {
        let dir = tempdir().unwrap();
        let mut mgr = RebootManager::load(dir.path());
        mgr.sync_policy_and_os_state(RebootPolicy::MaintenanceWindow, 120, false, 1000)
            .unwrap();
        mgr.mark_reboot_required(Some("job-2".to_string()), 1000)
            .unwrap();

        assert!(!mgr.should_execute_auto_reboot(2000, false));
        assert!(mgr.should_execute_auto_reboot(2000, true));
    }

    #[test]
    fn test_reboot_policy_grace_period_persistence_and_snooze_cap() {
        let dir = tempdir().unwrap();
        {
            let mut mgr = RebootManager::load(dir.path());
            mgr.sync_policy_and_os_state(RebootPolicy::GracePeriod, 120, false, 10_000)
                .unwrap();
            mgr.mark_reboot_required(Some("job-3".to_string()), 10_000)
                .unwrap();
            // 120 minutes = 7200 seconds -> deadline = 17_200
            assert_eq!(mgr.state().deadline_unix, Some(17_200));
            assert!(!mgr.should_execute_auto_reboot(15_000, false));
        }

        // Reload from disk (simulating service restart / user relog) -> deadline MUST persist unchanged!
        let mut reloaded = RebootManager::load(dir.path());
        assert!(reloaded.state().reboot_required);
        assert_eq!(reloaded.state().deadline_unix, Some(17_200));

        // Snooze 60m at t=11_000 -> snoozed_until = 14_600
        let s1 = reloaded.snooze(60, 11_000).unwrap();
        assert_eq!(s1.snoozed_until_unix, Some(14_600));

        // Snooze 240m at t=15_000 -> clamped to hard deadline 17_200!
        let s2 = reloaded.snooze(240, 15_000).unwrap();
        assert_eq!(s2.snoozed_until_unix, Some(17_200));

        // Snooze after deadline -> rejected with DeadlineExceeded
        assert_eq!(
            reloaded.snooze(60, 17_200),
            Err(RebootPolicyError::DeadlineExceeded)
        );
        assert!(reloaded.should_execute_auto_reboot(17_200, false));
    }
}
