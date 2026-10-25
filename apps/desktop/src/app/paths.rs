//! Filesystem locations the app reads and writes outside a project.
//!
//! Installed once at startup ([`init_globals`](crate::init_globals) uses
//! [`AppPaths::from_env`] unless a value is already set), so tests can point
//! everything at a scratch directory instead of the real home.

use std::path::PathBuf;

use gpui_kit::{App, Global};

use crate::app::prefs::NativePreferences;
use crate::project::personal::personal_root;

#[derive(Clone, Debug)]
pub struct AppPaths {
    /// Per-user config tree (`~/.config/based`): personal connections, `.env`.
    pub personal_root: PathBuf,
    /// `native_preferences.toml`.
    pub prefs_path: PathBuf,
}

impl AppPaths {
    pub fn from_env() -> Self {
        Self {
            personal_root: personal_root(),
            prefs_path: NativePreferences::prefs_path(),
        }
    }

    /// `updater_state.toml`, next to the preferences file.
    pub fn updater_state_path(&self) -> PathBuf {
        self.prefs_path.with_file_name("updater_state.toml")
    }

    pub fn get(cx: &App) -> &Self {
        cx.global::<Self>()
    }
}

impl Global for AppPaths {}
