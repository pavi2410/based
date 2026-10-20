//! Native app preferences persisted as TOML (theme, chrome layout, typography).

use super::chrome::ChromePrefs as Chrome;
pub use super::chrome::{
    ChromePrefs, CodeFontFamilyId, DensityPreset, FontWeightToken, SizeToken, UiFontFamilyId,
    apply_chrome, panel_header_h, sidebar_row_gap, sidebar_row_py,
};
use crate::theme::{DEFAULT_DARK_THEME, DEFAULT_LIGHT_THEME, apply_theme_names, preset_by_id};
use gpui_kit::component::{Size, Theme, ThemeMode};
use gpui_kit::{App, BorrowAppContext, FontWeight, Global, SharedString};
use serde::{Deserialize, Serialize};
use std::fs;
use std::path::PathBuf;

/// Define `fn name(value, cx)` that updates one field through `$update`, skipping no-op writes.
macro_rules! pref_setter {
    ($name:ident, $field:ident: $ty:ty, $update:ident) => {
        pub fn $name($field: $ty, cx: &mut App) {
            $update(
                |p| {
                    if p.$field == $field {
                        return false;
                    }
                    p.$field = $field;
                    true
                },
                cx,
            );
        }
    };
}

pub(super) fn default_true() -> bool {
    true
}

mod appearance;
mod table;
mod update;

pub use appearance::*;
pub use table::*;
pub use update::*;

#[derive(Clone, Serialize, Deserialize, PartialEq)]
pub struct NativePreferences {
    #[serde(default)]
    pub appearance_mode: AppearanceMode,
    #[serde(default = "default_light_theme")]
    pub light_theme: String,
    #[serde(default = "default_dark_theme")]
    pub dark_theme: String,
    #[serde(default)]
    pub sidebar_collapsed: bool,
    #[serde(default, flatten)]
    pub chrome: Chrome,
    #[serde(default = "default_page_size")]
    pub page_size: u64,
    #[serde(default = "default_query_timeout_secs")]
    pub query_timeout_secs: u32,
    #[serde(default)]
    pub table_prefs: TablePreferences,
    #[serde(default)]
    pub update: UpdatePreferences,
    #[serde(default)]
    pub last_opened_project: Option<PathBuf>,
    #[serde(default)]
    pub recent_projects: Vec<PathBuf>,
}

impl Default for NativePreferences {
    fn default() -> Self {
        Self {
            appearance_mode: AppearanceMode::Dark,
            light_theme: default_light_theme(),
            dark_theme: default_dark_theme(),
            sidebar_collapsed: false,
            chrome: Chrome::default(),
            page_size: DEFAULT_PAGE_SIZE,
            query_timeout_secs: DEFAULT_QUERY_TIMEOUT_SECS,
            table_prefs: TablePreferences::default(),
            update: UpdatePreferences::default(),
            last_opened_project: None,
            recent_projects: Vec::new(),
        }
    }
}

impl Global for NativePreferences {}

fn migrate_legacy_theme_names(prefs: &mut NativePreferences) {
    if prefs.light_theme == "Default Light" {
        prefs.light_theme = DEFAULT_LIGHT_THEME.to_string();
    }
    if prefs.dark_theme == "Default Dark" {
        prefs.dark_theme = DEFAULT_DARK_THEME.to_string();
    }
}

fn migrate_legacy_chrome(prefs: &mut NativePreferences) {
    prefs.chrome.sync_density_preset();
}

impl NativePreferences {
    pub fn prefs_path() -> PathBuf {
        let base = dirs::preference_dir()
            .unwrap_or_else(|| dirs::home_dir().unwrap_or_default().join(".config"))
            .join("based");
        let _ = fs::create_dir_all(&base);
        base.join("native_preferences.toml")
    }

    pub fn load() -> Self {
        let path = Self::prefs_path();
        let raw = fs::read_to_string(&path).unwrap_or_default();
        let mut prefs: Self = match raw.is_empty() {
            true => Self::default(),
            false => match toml::from_str(&raw) {
                Ok(p) => p,
                Err(e) => {
                    log::debug!("native prefs: using defaults ({path:?}: {e})");
                    Self::default()
                }
            },
        };
        if !raw.contains("appearance_mode")
            && let Ok(value) = toml::from_str::<toml::Value>(&raw)
            && let Some(mode) = value.get("theme_mode").and_then(|v| v.as_str())
        {
            prefs.appearance_mode = match mode {
                "light" => AppearanceMode::Light,
                "dark" => AppearanceMode::Dark,
                _ => AppearanceMode::Dark,
            };
        }
        if !raw.contains("light_theme")
            && let Ok(value) = toml::from_str::<toml::Value>(&raw)
            && let Some(preset_id) = value.get("theme_preset").and_then(|v| v.as_str())
        {
            let preset_id = match preset_id {
                "neutral" => "based",
                _ => preset_id,
            };
            if let Some(preset) = preset_by_id(preset_id) {
                prefs.light_theme = preset.light_name.to_string();
                prefs.dark_theme = preset.dark_name.to_string();
            }
        }
        migrate_legacy_theme_names(&mut prefs);
        migrate_legacy_chrome(&mut prefs);
        apply_dev_update_policy(&mut prefs.update);
        prefs
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
        if let Err(e) = fs::write(path, encoded) {
            log::warn!("native prefs save: {e:#}");
        }
    }

    /// Forget the last-opened path so Dock relaunch does not reopen it.
    /// Recent projects are left intact.
    pub fn forget_last_opened_project(&mut self) {
        self.last_opened_project = None;
    }
}

fn chrome_prefs(cx: &App) -> ChromePrefs {
    cx.global::<NativePreferences>().chrome.clone()
}

fn update_chrome(update: impl FnOnce(&mut ChromePrefs) -> bool, cx: &mut App) {
    let changed = cx.update_global(|p: &mut NativePreferences, _| {
        let changed = update(&mut p.chrome);
        if changed {
            p.chrome.sync_density_preset();
            p.save_best_effort();
        }
        changed
    });
    if changed {
        let prefs = chrome_prefs(cx);
        apply_chrome(&prefs, cx);
    }
}

/// Load persisted prefs into a global and apply appearance + theme pair.
pub fn install(cx: &mut App) {
    let prefs = NativePreferences::load();
    let light = prefs.light_theme.clone();
    let dark = prefs.dark_theme.clone();
    let chrome = prefs.chrome.clone();
    cx.set_global(prefs);
    if let Err(err) = apply_theme_names(&light, &dark, cx) {
        log::warn!("theme pair on startup: {err:#}");
        let _ = apply_theme_names(DEFAULT_LIGHT_THEME, DEFAULT_DARK_THEME, cx);
    }
    reapply_appearance(None, cx);
    apply_chrome(&chrome, cx);
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

const MAX_RECENT_PROJECTS: usize = 10;

/// Record a successfully opened project for Dock relaunch and future Open Recent UI.
pub fn record_opened_project(path: PathBuf, cx: &mut App) {
    cx.update_global(|p: &mut NativePreferences, _| {
        p.last_opened_project = Some(path.clone());
        p.recent_projects.retain(|entry| entry != &path);
        p.recent_projects.insert(0, path);
        p.recent_projects.truncate(MAX_RECENT_PROJECTS);
        p.save_best_effort();
    });
}

/// Clear `last_opened_project` after Close Project. Keeps `recent_projects`.
pub fn clear_last_opened_project(cx: &mut App) {
    cx.update_global(|p: &mut NativePreferences, _| {
        if p.last_opened_project.is_none() {
            return;
        }
        p.forget_last_opened_project();
        p.save_best_effort();
    });
}

#[cfg(test)]
mod last_opened_project_tests {
    use super::*;

    #[test]
    fn forget_last_opened_keeps_recent_projects() {
        let mut prefs = NativePreferences::default();
        let path = PathBuf::from("/proj");
        prefs.last_opened_project = Some(path.clone());
        prefs.recent_projects.push(path);
        prefs.forget_last_opened_project();
        assert!(prefs.last_opened_project.is_none());
        assert_eq!(prefs.recent_projects, [PathBuf::from("/proj")]);
    }
}
