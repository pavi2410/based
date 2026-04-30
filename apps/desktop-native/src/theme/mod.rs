//! Install the Based graphite theme pair into gpui-component's registry and active slot.

use anyhow::Context as _;
use gpui::{App, SharedString};
use gpui_component::{Theme, ThemeRegistry};

pub fn install_based_theme(cx: &mut App) -> anyhow::Result<()> {
    ThemeRegistry::global_mut(cx)
        .load_themes_from_str(include_str!("based_theme.json"))
        .context("load Based theme bundle")?;

    let reg = ThemeRegistry::global(cx);
    let light = reg
        .themes()
        .get(&SharedString::from("Based Light"))
        .context("missing theme name \"Based Light\" after JSON load")?
        .clone();
    let dark = reg
        .themes()
        .get(&SharedString::from("Based Dark"))
        .context("missing theme name \"Based Dark\" after JSON load")?
        .clone();

    Theme::global_mut(cx).light_theme = light;
    Theme::global_mut(cx).dark_theme = dark;
    Ok(())
}
