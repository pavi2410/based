//! Native app preferences persisted as TOML (theme, chrome layout).

use std::path::PathBuf;

use gpui::{App, BorrowAppContext, Global};
use gpui_component::{Theme, ThemeMode};
use serde::{Deserialize, Serialize};

#[derive(Clone, Serialize, Deserialize, PartialEq)]
pub struct NativePreferences {
    #[serde(default)]
    pub theme_mode: ThemeMode,
    #[serde(default)]
    pub sidebar_collapsed: bool,
}

impl Default for NativePreferences {
    fn default() -> Self {
        Self {
            theme_mode: ThemeMode::Dark,
            sidebar_collapsed: false,
        }
    }
}

impl Global for NativePreferences {}

impl NativePreferences {
    pub fn prefs_path() -> PathBuf {
        let base = dirs::preference_dir()
            .unwrap_or_else(|| dirs::home_dir().unwrap_or_default().join(".config"))
            .join("based");
        let _ = std::fs::create_dir_all(&base);
        base.join("native_preferences.toml")
    }

    pub fn load() -> Self {
        let path = Self::prefs_path();
        match std::fs::read_to_string(&path).and_then(|s| {
            toml::from_str(&s).map_err(|e| std::io::Error::new(std::io::ErrorKind::InvalidData, e))
        }) {
            Ok(p) => p,
            Err(e) => {
                log::debug!("native prefs: using defaults ({path:?}: {e})");
                Self::default()
            }
        }
    }

    pub fn save_best_effort(&self) {
        let path = Self::prefs_path();
        let encoded = match toml::to_string_pretty(self) {
            Ok(s) => s,
            Err(e) => {
                log::warn!("native prefs serialize: {e:#}");
                return;
            }
        };
        if let Err(e) = std::fs::write(path, encoded) {
            log::warn!("native prefs save: {e:#}");
        }
    }
}

/// Load persisted prefs into a global and apply appearance.
pub fn install(cx: &mut App) {
    let prefs = NativePreferences::load();
    Theme::change(prefs.theme_mode, None, cx);
    cx.set_global(prefs);
}

pub fn collapsed_from(cx: &App) -> bool {
    cx.global::<NativePreferences>().sidebar_collapsed
}

pub fn set_sidebar(collapsed: bool, cx: &mut App) {
    cx.update_global(|p: &mut NativePreferences, _| {
        if p.sidebar_collapsed == collapsed {
            return;
        }
        p.sidebar_collapsed = collapsed;
        p.save_best_effort();
    });
}

pub fn cycle_theme(cx: &mut App) {
    let next = match Theme::global(cx).mode {
        ThemeMode::Dark => ThemeMode::Light,
        ThemeMode::Light => ThemeMode::Dark,
    };
    Theme::change(next, None, cx);
    cx.update_global(|p: &mut NativePreferences, _| {
        p.theme_mode = next;
        p.save_best_effort();
    });
}
