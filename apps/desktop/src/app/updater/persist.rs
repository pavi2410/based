use std::path::PathBuf;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use serde::{Deserialize, Serialize};

use super::config;
use super::log::{debug as udebug, warn as uwarn};

#[derive(Clone, Debug, Default, Serialize, Deserialize, PartialEq)]
pub struct UpdaterStateFile {
    #[serde(default)]
    pub last_check_at: Option<u64>,
    #[serde(default)]
    pub dismissed_version: Option<String>,
    #[serde(default)]
    pub pending_release_notes_version: Option<String>,
}

impl UpdaterStateFile {
    pub fn path() -> PathBuf {
        crate::app::prefs::NativePreferences::prefs_path()
            .parent()
            .map(|p| p.join("updater_state.toml"))
            .unwrap_or_else(|| PathBuf::from("updater_state.toml"))
    }

    pub fn load() -> Self {
        let path = Self::path();
        let raw = std::fs::read_to_string(&path).unwrap_or_default();
        if raw.is_empty() {
            udebug(format!("state load: empty ({path:?})"));
            return Self::default();
        }
        match toml::from_str::<UpdaterStateFile>(&raw) {
            Ok(state) => {
                udebug(format!(
                    "state load: last_check_at={:?} dismissed={:?} pending_notes={:?}",
                    state.last_check_at,
                    state.dismissed_version,
                    state.pending_release_notes_version
                ));
                state
            }
            Err(e) => {
                uwarn(format!("state load ({path:?}): {e}"));
                Self::default()
            }
        }
    }

    pub fn save_best_effort(&self) {
        let path = Self::path();
        if let Some(parent) = path.parent() {
            let _ = std::fs::create_dir_all(parent);
        }
        match toml::to_string_pretty(self) {
            Ok(encoded) => {
                if let Err(e) = std::fs::write(&path, encoded) {
                    uwarn(format!("state save ({path:?}): {e:#}"));
                } else {
                    udebug(format!("state save: ok ({path:?})"));
                }
            }
            Err(e) => uwarn(format!("state serialize: {e:#}")),
        }
    }

    pub fn mark_checked_now(&mut self) {
        self.last_check_at = Some(now_unix());
        self.save_best_effort();
    }

    pub fn set_pending_release_notes(&mut self, version: &str) {
        self.pending_release_notes_version = Some(version.to_string());
        self.save_best_effort();
    }

    pub fn take_pending_release_notes(&mut self) -> Option<String> {
        let v = self.pending_release_notes_version.take();
        if v.is_some() {
            self.save_best_effort();
        }
        v
    }

    pub fn clear_pending_release_notes(&mut self) {
        if self.pending_release_notes_version.is_some() {
            self.pending_release_notes_version = None;
            self.save_best_effort();
        }
    }
}

pub fn now_unix() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or(Duration::ZERO)
        .as_secs()
}

pub fn interval_secs(interval: crate::app::prefs::UpdateCheckInterval) -> u64 {
    match interval {
        crate::app::prefs::UpdateCheckInterval::Daily => 24 * 3600,
        crate::app::prefs::UpdateCheckInterval::Weekly => 7 * 24 * 3600,
        crate::app::prefs::UpdateCheckInterval::Monthly => 30 * 24 * 3600,
    }
}

pub fn should_run_periodic_check(
    state: &UpdaterStateFile,
    interval: crate::app::prefs::UpdateCheckInterval,
) -> bool {
    let Some(last) = state.last_check_at else {
        return true;
    };
    now_unix().saturating_sub(last) >= interval_secs(interval)
}

pub fn startup_check_stale(state: &UpdaterStateFile) -> bool {
    // Re-check on startup if last check was more than 6 hours ago.
    let Some(last) = state.last_check_at else {
        return true;
    };
    now_unix().saturating_sub(last) >= 6 * 3600
}

/// Dev builds skip network updater entirely.
pub fn updates_enabled() -> bool {
    !config::is_dev_build()
}
