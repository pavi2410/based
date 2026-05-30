//! Theme registry setup and preset application.

mod controls;
mod presets;

pub use controls::{
    ThemeNameItem, ThemePreviewAxis, ThemePreviewSession, load_bundled_themes, theme_name_select,
    theme_onboarding_picker,
};
pub use presets::{
    DEFAULT_DARK_THEME, DEFAULT_LIGHT_THEME, DEFAULT_PRESET_ID, preset_by_id, preset_id_for_pair,
};

use anyhow::Context as _;
use gpui::{App, SharedString};
use gpui_component::{Theme, ThemeRegistry};

/// Load bundled theme presets into the registry (does not apply active pair).
pub fn register_themes(cx: &mut App) -> anyhow::Result<()> {
    load_bundled_themes(cx);
    Ok(())
}

/// Apply light and dark registry themes to the global theme pair (does not persist prefs).
pub fn apply_theme_names(light: &str, dark: &str, cx: &mut App) -> anyhow::Result<()> {
    let reg = ThemeRegistry::global(cx);
    let light = reg
        .themes()
        .get(&SharedString::from(light))
        .with_context(|| format!("missing light theme {light:?}"))?
        .clone();
    let dark = reg
        .themes()
        .get(&SharedString::from(dark))
        .with_context(|| format!("missing dark theme {dark:?}"))?
        .clone();

    Theme::global_mut(cx).light_theme = light;
    Theme::global_mut(cx).dark_theme = dark;
    Ok(())
}

/// Apply a paired preset id to the global theme pair (onboarding).
pub fn apply_preset_pair(preset_id: &str, cx: &mut App) -> anyhow::Result<()> {
    let preset =
        preset_by_id(preset_id).with_context(|| format!("unknown theme preset {preset_id:?}"))?;
    apply_theme_names(preset.light_name, preset.dark_name, cx)
}
