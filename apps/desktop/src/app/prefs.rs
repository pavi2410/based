//! Native app preferences persisted as TOML (theme, chrome layout, typography).

use std::path::PathBuf;

use gpui::{px, App, BorrowAppContext, Global};
use gpui_component::{Theme, ThemeMode};
use serde::{Deserialize, Serialize};

/// UI base font size from `based_theme.json` (`font.size`).
pub const DEFAULT_UI_FONT_SIZE: f32 = 14.0;
/// Monospace font size from `based_theme.json` (`mono_font.size`).
pub const DEFAULT_MONO_FONT_SIZE: f32 = 14.0;

const UI_FONT_MIN: f32 = 10.0;
const UI_FONT_MAX: f32 = 24.0;
const MONO_FONT_MIN: f32 = 10.0;
const MONO_FONT_MAX: f32 = 22.0;

fn default_ui_font_size() -> f32 {
    DEFAULT_UI_FONT_SIZE
}

fn default_mono_font_size() -> f32 {
    DEFAULT_MONO_FONT_SIZE
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
}

impl Default for NativePreferences {
    fn default() -> Self {
        Self {
            theme_mode: ThemeMode::Dark,
            sidebar_collapsed: false,
            ui_font_size: DEFAULT_UI_FONT_SIZE,
            mono_font_size: DEFAULT_MONO_FONT_SIZE,
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
        (clamp_ui_font(prefs.ui_font_size), clamp_mono_font(prefs.mono_font_size))
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
    apply_font_sizes(cx);
}
