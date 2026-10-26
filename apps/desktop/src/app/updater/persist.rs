use std::path::PathBuf;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use serde::{Deserialize, Serialize};

use super::super::prefs::UpdateCheckInterval;
use super::super::toml_file;

use super::log::{debug as udebug, warn as uwarn};

#[derive(Clone, Debug, Default, Serialize, Deserialize, PartialEq)]
pub struct UpdaterStateFile {
    #[serde(default)]
    pub last_check_at: Option<u64>,
    #[serde(default)]
    pub dismissed_version: Option<String>,
    #[serde(default)]
    pub pending_release_notes_version: Option<String>,
    #[serde(skip)]
    path: PathBuf,
}

impl UpdaterStateFile {
    pub fn load_from(path: PathBuf) -> Self {
        let mut state: Self = toml_file::parse(&toml_file::read(&path), &path);
        udebug(format!(
            "state load: last_check_at={:?} dismissed={:?} pending_notes={:?}",
            state.last_check_at, state.dismissed_version, state.pending_release_notes_version
        ));
        state.path = path;
        state
    }

    pub fn save_best_effort(&self) {
        match toml_file::write(&self.path, self) {
            Ok(()) => udebug(format!("state save: ok ({:?})", self.path)),
            Err(e) => uwarn(format!("state save: {e}")),
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

pub fn interval_secs(interval: UpdateCheckInterval) -> u64 {
    match interval {
        UpdateCheckInterval::Daily => 24 * 3600,
        UpdateCheckInterval::Weekly => 7 * 24 * 3600,
        UpdateCheckInterval::Monthly => 30 * 24 * 3600,
    }
}

pub fn should_run_periodic_check(state: &UpdaterStateFile, interval: UpdateCheckInterval) -> bool {
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn state_round_trips_through_the_given_path() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("nested").join("updater_state.toml");
        let mut state = UpdaterStateFile::load_from(path.clone());
        assert_eq!(state.last_check_at, None);
        state.set_pending_release_notes("1.2.3");
        let mut again = UpdaterStateFile::load_from(path);
        assert_eq!(again.take_pending_release_notes().as_deref(), Some("1.2.3"));
        assert_eq!(again.pending_release_notes_version, None);
    }
}
