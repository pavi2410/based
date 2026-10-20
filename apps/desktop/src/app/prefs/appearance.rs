//! Theme, typography and density preferences.

use super::*;

pub(super) fn default_light_theme() -> String {
    DEFAULT_LIGHT_THEME.to_string()
}

pub(super) fn default_dark_theme() -> String {
    DEFAULT_DARK_THEME.to_string()
}

#[derive(Clone, Copy, Debug, Serialize, Deserialize, PartialEq, Eq, Default)]
#[serde(rename_all = "snake_case")]
pub enum AppearanceMode {
    Light,
    Dark,
    #[default]
    System,
}

impl AppearanceMode {
    pub const ALL: [Self; 3] = [Self::Light, Self::Dark, Self::System];

    pub fn label(self) -> &'static str {
        match self {
            Self::Light => "Light",
            Self::Dark => "Dark",
            Self::System => "System",
        }
    }

    pub fn storage_key(self) -> &'static str {
        match self {
            Self::Light => "light",
            Self::Dark => "dark",
            Self::System => "system",
        }
    }

    pub fn from_storage_key(key: &str) -> Option<Self> {
        match key {
            "light" => Some(Self::Light),
            "dark" => Some(Self::Dark),
            "system" => Some(Self::System),
            _ => None,
        }
    }
}

pub fn density_preset(cx: &App) -> DensityPreset {
    cx.global::<NativePreferences>().chrome.density_preset
}

pub fn ui_font_family(cx: &App) -> SharedString {
    cx.global::<NativePreferences>().chrome.ui.family.resolve()
}

pub fn ui_font_weight(cx: &App) -> FontWeight {
    cx.global::<NativePreferences>().chrome.ui.weight.to_gpui()
}

pub fn ui_size_token(cx: &App) -> SizeToken {
    cx.global::<NativePreferences>().chrome.ui.size
}

pub fn code_font_family(cx: &App) -> SharedString {
    cx.global::<NativePreferences>()
        .chrome
        .editor
        .family
        .resolve()
}

pub fn code_font_weight(cx: &App) -> FontWeight {
    cx.global::<NativePreferences>()
        .chrome
        .editor
        .weight
        .to_gpui()
}

pub fn editor_size_token(cx: &App) -> SizeToken {
    cx.global::<NativePreferences>().chrome.editor.size
}

pub fn ui_component_size(cx: &App) -> Size {
    ui_size_token(cx).to_component_size()
}

pub fn apply_density_preset(preset: DensityPreset, cx: &mut App) {
    if preset == DensityPreset::Custom {
        return;
    }
    update_chrome(
        |c| {
            let before = c.clone();
            c.apply_density_preset(preset);
            before != *c
        },
        cx,
    );
}

pub fn set_ui_font_family(family: UiFontFamilyId, cx: &mut App) {
    update_chrome(
        |c| {
            if c.ui.family == family {
                return false;
            }
            c.ui.family = family;
            true
        },
        cx,
    );
}

pub fn set_ui_font_weight(weight: FontWeightToken, cx: &mut App) {
    update_chrome(
        |c| {
            if c.ui.weight == weight {
                return false;
            }
            c.ui.weight = weight;
            true
        },
        cx,
    );
}

pub fn set_ui_size(size: SizeToken, cx: &mut App) {
    update_chrome(
        |c| {
            if c.ui.size == size {
                return false;
            }
            c.ui.size = size;
            true
        },
        cx,
    );
}

pub fn set_editor_font_family(family: CodeFontFamilyId, cx: &mut App) {
    update_chrome(
        |c| {
            if c.editor.family == family {
                return false;
            }
            c.editor.family = family;
            true
        },
        cx,
    );
}

pub fn set_editor_font_weight(weight: FontWeightToken, cx: &mut App) {
    update_chrome(
        |c| {
            if c.editor.weight == weight {
                return false;
            }
            c.editor.weight = weight;
            true
        },
        cx,
    );
}

pub fn set_editor_size(size: SizeToken, cx: &mut App) {
    update_chrome(
        |c| {
            if c.editor.size == size {
                return false;
            }
            c.editor.size = size;
            true
        },
        cx,
    );
}

pub fn appearance_mode(cx: &App) -> AppearanceMode {
    cx.global::<NativePreferences>().appearance_mode
}

pub fn light_theme_name(cx: &App) -> &str {
    &cx.global::<NativePreferences>().light_theme
}

pub fn dark_theme_name(cx: &App) -> &str {
    &cx.global::<NativePreferences>().dark_theme
}

pub fn apply_appearance(mode: AppearanceMode, window: Option<&mut gpui_kit::Window>, cx: &mut App) {
    match mode {
        AppearanceMode::Light => Theme::change(ThemeMode::Light, window, cx),
        AppearanceMode::Dark => Theme::change(ThemeMode::Dark, window, cx),
        AppearanceMode::System => Theme::sync_system_appearance(window, cx),
    }
    cx.update_global(|p: &mut NativePreferences, _| {
        p.appearance_mode = mode;
        p.save_best_effort();
    });
    let chrome = chrome_prefs(cx);
    apply_chrome(&chrome, cx);
    cx.refresh_windows();
}

pub fn reapply_appearance(window: Option<&mut gpui_kit::Window>, cx: &mut App) {
    let mode = appearance_mode(cx);
    apply_appearance(mode, window, cx);
}

pub fn apply_theme_pair(
    light: &str,
    dark: &str,
    window: Option<&mut gpui_kit::Window>,
    cx: &mut App,
) {
    if let Err(err) = apply_theme_names(light, dark, cx) {
        log::warn!("apply theme pair ({light:?}, {dark:?}): {err:#}");
        return;
    }
    cx.update_global(|p: &mut NativePreferences, _| {
        let mut changed = false;
        if p.light_theme != light {
            p.light_theme = light.to_string();
            changed = true;
        }
        if p.dark_theme != dark {
            p.dark_theme = dark.to_string();
            changed = true;
        }
        if changed {
            p.save_best_effort();
        }
    });
    reapply_appearance(window, cx);
}

pub fn apply_light_theme(name: &str, window: Option<&mut gpui_kit::Window>, cx: &mut App) {
    let dark = dark_theme_name(cx).to_string();
    apply_theme_pair(name, &dark, window, cx);
}

pub fn apply_dark_theme(name: &str, window: Option<&mut gpui_kit::Window>, cx: &mut App) {
    let light = light_theme_name(cx).to_string();
    apply_theme_pair(&light, name, window, cx);
}

pub fn cycle_theme(cx: &mut App) {
    let next = match appearance_mode(cx) {
        AppearanceMode::Light => AppearanceMode::Dark,
        AppearanceMode::Dark => AppearanceMode::System,
        AppearanceMode::System => AppearanceMode::Light,
    };
    apply_appearance(next, None, cx);
}
