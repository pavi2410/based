//! Native app preferences persisted as TOML (theme, chrome layout, typography).

use std::path::PathBuf;

use gpui::{App, BorrowAppContext, Global, px};
use gpui_component::{Theme, ThemeMode};
use serde::{Deserialize, Serialize};

/// UI base font size from `based_theme.json` (`font.size`).
pub const DEFAULT_UI_FONT_SIZE: f32 = 14.0;
/// Monospace font size from `based_theme.json` (`mono_font.size`).
pub const DEFAULT_MONO_FONT_SIZE: f32 = 14.0;
/// Default SQL data viewer page size (rows per fetch).
pub const DEFAULT_PAGE_SIZE: u64 = 500;
/// Default query timeout shown in settings (seconds); execution wiring is future work.
pub const DEFAULT_QUERY_TIMEOUT_SECS: u32 = 30;

const UI_FONT_MIN: f32 = 10.0;
const PAGE_SIZE_MIN: u64 = 50;
const PAGE_SIZE_MAX: u64 = 5000;
const QUERY_TIMEOUT_MIN: u32 = 5;
const QUERY_TIMEOUT_MAX: u32 = 600;
const UI_FONT_MAX: f32 = 24.0;
const MONO_FONT_MIN: f32 = 10.0;
const MONO_FONT_MAX: f32 = 22.0;

fn default_ui_font_size() -> f32 {
    DEFAULT_UI_FONT_SIZE
}

fn default_mono_font_size() -> f32 {
    DEFAULT_MONO_FONT_SIZE
}

fn default_page_size() -> u64 {
    DEFAULT_PAGE_SIZE
}

fn default_query_timeout_secs() -> u32 {
    DEFAULT_QUERY_TIMEOUT_SECS
}

/// Data grid row density (monospace cells).
#[derive(Clone, Copy, Debug, Serialize, Deserialize, PartialEq, Eq, Default)]
#[serde(rename_all = "snake_case")]
pub enum TableDensity {
    #[default]
    Compact,
    Comfortable,
}

#[derive(Clone, Serialize, Deserialize, PartialEq)]
pub struct NativePreferences {
    #[serde(default)]
    pub theme_mode: ThemeMode,
    #[serde(default)]
    pub sidebar_collapsed: bool,
    #[serde(default = "default_ui_font_size")]
    pub ui_font_size: f32,
    #[serde(default = "default_mono_font_size")]
    pub mono_font_size: f32,
    #[serde(default = "default_page_size")]
    pub page_size: u64,
    #[serde(default = "default_query_timeout_secs")]
    pub query_timeout_secs: u32,
    #[serde(default)]
    pub table_density: TableDensity,
}

impl Default for NativePreferences {
    fn default() -> Self {
        Self {
            theme_mode: ThemeMode::Dark,
            sidebar_collapsed: false,
            ui_font_size: DEFAULT_UI_FONT_SIZE,
            mono_font_size: DEFAULT_MONO_FONT_SIZE,
            page_size: DEFAULT_PAGE_SIZE,
            query_timeout_secs: DEFAULT_QUERY_TIMEOUT_SECS,
            table_density: TableDensity::Compact,
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
    apply_font_sizes(cx);
}

pub fn ui_font_size(cx: &App) -> f32 {
    cx.global::<NativePreferences>().ui_font_size
}

pub fn mono_font_size(cx: &App) -> f32 {
    cx.global::<NativePreferences>().mono_font_size
}

pub fn page_size(cx: &App) -> u64 {
    cx.global::<NativePreferences>().page_size
}

pub fn query_timeout_secs(cx: &App) -> u32 {
    cx.global::<NativePreferences>().query_timeout_secs
}

pub fn table_density(cx: &App) -> TableDensity {
    cx.global::<NativePreferences>().table_density
}

pub fn set_table_density(density: TableDensity, cx: &mut App) {
    let changed = cx.update_global(|p: &mut NativePreferences, _| {
        if p.table_density == density {
            return false;
        }
        p.table_density = density;
        p.save_best_effort();
        true
    });
    if changed {
        cx.refresh_windows();
    }
}

fn clamp_page_size(size: u64) -> u64 {
    size.clamp(PAGE_SIZE_MIN, PAGE_SIZE_MAX)
}

fn clamp_query_timeout(secs: u32) -> u32 {
    secs.clamp(QUERY_TIMEOUT_MIN, QUERY_TIMEOUT_MAX)
}

pub fn set_page_size(size: u64, cx: &mut App) {
    let size = clamp_page_size(size);
    cx.update_global(|p: &mut NativePreferences, _| {
        if p.page_size == size {
            return;
        }
        p.page_size = size;
        p.save_best_effort();
    });
}

pub fn set_query_timeout_secs(secs: u32, cx: &mut App) {
    let secs = clamp_query_timeout(secs);
    cx.update_global(|p: &mut NativePreferences, _| {
        if p.query_timeout_secs == secs {
            return;
        }
        p.query_timeout_secs = secs;
        p.save_best_effort();
    });
}

fn clamp_ui_font(size: f32) -> f32 {
    size.clamp(UI_FONT_MIN, UI_FONT_MAX)
}

fn clamp_mono_font(size: f32) -> f32 {
    size.clamp(MONO_FONT_MIN, MONO_FONT_MAX)
}

/// Apply persisted font sizes to the active theme and refresh all windows.
pub fn apply_font_sizes(cx: &mut App) {
    let (ui, mono) = {
        let prefs = cx.global::<NativePreferences>();
        (
            clamp_ui_font(prefs.ui_font_size),
            clamp_mono_font(prefs.mono_font_size),
        )
    };
    let theme = Theme::global_mut(cx);
    theme.font_size = px(ui);
    theme.mono_font_size = px(mono);
    cx.refresh_windows();
}

pub fn set_ui_font_size(size: f32, cx: &mut App) {
    let size = clamp_ui_font(size);
    let changed = cx.update_global(|p: &mut NativePreferences, _| {
        if (p.ui_font_size - size).abs() < f32::EPSILON {
            return false;
        }
        p.ui_font_size = size;
        p.save_best_effort();
        true
    });
    if changed {
        apply_font_sizes(cx);
    }
}

pub fn set_mono_font_size(size: f32, cx: &mut App) {
    let size = clamp_mono_font(size);
    let changed = cx.update_global(|p: &mut NativePreferences, _| {
        if (p.mono_font_size - size).abs() < f32::EPSILON {
            return false;
        }
        p.mono_font_size = size;
        p.save_best_effort();
        true
    });
    if changed {
        apply_font_sizes(cx);
    }
}

pub fn adjust_ui_font_size(delta: f32, cx: &mut App) {
    set_ui_font_size(ui_font_size(cx) + delta, cx);
}

pub fn adjust_mono_font_size(delta: f32, cx: &mut App) {
    set_mono_font_size(mono_font_size(cx) + delta, cx);
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

/// Switch theme mode, persist, and repaint every window.
pub fn apply_theme(mode: ThemeMode, cx: &mut App) {
    Theme::change(mode, None, cx);
    cx.update_global(|p: &mut NativePreferences, _| {
        p.theme_mode = mode;
        p.save_best_effort();
    });
    apply_font_sizes(cx);
    cx.refresh_windows();
}

pub fn cycle_theme(cx: &mut App) {
    let next = match Theme::global(cx).mode {
        ThemeMode::Dark => ThemeMode::Light,
        ThemeMode::Light => ThemeMode::Dark,
    };
    apply_theme(next, cx);
}
