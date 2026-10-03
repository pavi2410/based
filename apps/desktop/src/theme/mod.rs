//! Theme registry setup and preset application.

mod controls;
mod presets;

pub use controls::{ThemeNameItem, load_bundled_themes};
pub use presets::{DEFAULT_DARK_THEME, DEFAULT_LIGHT_THEME, preset_by_id};

use anyhow::Context as _;
use gpui_kit::component::{Theme, ThemeRegistry};
use gpui_kit::{App, SharedString};

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
