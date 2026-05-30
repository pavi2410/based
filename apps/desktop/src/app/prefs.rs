//! Native app preferences persisted as TOML (theme, chrome layout, typography).

use std::path::PathBuf;

use gpui::{App, BorrowAppContext, FontWeight, Global, SharedString};
use gpui_component::{Size, Theme, ThemeMode};
use serde::{Deserialize, Serialize};

pub use super::chrome::{
    ChromePrefs, CodeFontFamilyId, DensityPreset, FontWeightToken, SizeToken, UiFontFamilyId,
    apply_chrome, panel_header_h, sidebar_row_gap, sidebar_row_py,
};

use super::chrome::ChromePrefs as Chrome;

/// Default SQL data viewer page size (rows per fetch).
pub const DEFAULT_PAGE_SIZE: u64 = 500;
/// Default query timeout shown in settings (seconds); execution wiring is future work.
pub const DEFAULT_QUERY_TIMEOUT_SECS: u32 = 30;

const PAGE_SIZE_MIN: u64 = 50;
const PAGE_SIZE_MAX: u64 = 5000;
const QUERY_TIMEOUT_MIN: u32 = 5;
const QUERY_TIMEOUT_MAX: u32 = 600;

fn default_page_size() -> u64 {
    DEFAULT_PAGE_SIZE
}

fn default_query_timeout_secs() -> u32 {
    DEFAULT_QUERY_TIMEOUT_SECS
}

/// Interaction and chrome toggles for data grids (gpui-component DataTable).
#[derive(Clone, Copy, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub struct TablePreferences {
    #[serde(default = "default_true")]
    pub stripe: bool,
    #[serde(default = "default_true")]
    pub sortable: bool,
    #[serde(default = "default_true")]
    pub col_resizable: bool,
    #[serde(default = "default_true")]
    pub col_movable: bool,
    #[serde(default = "default_true")]
    pub row_selectable: bool,
    #[serde(default = "default_true")]
    pub cell_selectable: bool,
    #[serde(default = "default_true")]
    pub loop_selection: bool,
}

fn default_true() -> bool {
    true
}

#[derive(Clone, Copy, Debug, Default, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum UpdateCheckInterval {
    #[default]
    Daily,
    Weekly,
    Monthly,
}

impl UpdateCheckInterval {
    pub const ALL: [Self; 3] = [Self::Daily, Self::Weekly, Self::Monthly];

    pub fn label(self) -> &'static str {
        match self {
            Self::Daily => "Daily",
            Self::Weekly => "Weekly",
            Self::Monthly => "Monthly",
        }
    }

    pub fn storage_key(self) -> &'static str {
        match self {
            Self::Daily => "daily",
            Self::Weekly => "weekly",
            Self::Monthly => "monthly",
        }
    }

    pub fn from_storage_key(key: &str) -> Option<Self> {
        match key {
            "daily" => Some(Self::Daily),
            "weekly" => Some(Self::Weekly),
            "monthly" => Some(Self::Monthly),
            _ => None,
        }
    }
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub struct UpdatePreferences {
    #[serde(default = "default_true")]
    pub check_at_startup: bool,
    #[serde(default = "default_true")]
    pub auto_check: bool,
    #[serde(default)]
    pub check_interval: UpdateCheckInterval,
    #[serde(default = "default_true")]
    pub auto_download: bool,
    #[serde(default = "default_true")]
    pub show_release_notes_after_update: bool,
    #[serde(default)]
    pub include_prereleases: bool,
}

impl Default for UpdatePreferences {
    fn default() -> Self {
        Self {
            check_at_startup: true,
            auto_check: true,
            check_interval: UpdateCheckInterval::Daily,
            auto_download: true,
            show_release_notes_after_update: true,
            include_prereleases: false,
        }
    }
}

fn default_light_theme() -> String {
    crate::theme::DEFAULT_LIGHT_THEME.to_string()
}

fn default_dark_theme() -> String {
    crate::theme::DEFAULT_DARK_THEME.to_string()
}

impl Default for TablePreferences {
    fn default() -> Self {
        Self {
            stripe: true,
            sortable: true,
            col_resizable: true,
            col_movable: true,
            row_selectable: true,
            cell_selectable: true,
            loop_selection: true,
        }
    }
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
    pub onboarding_completed: bool,
    #[serde(default)]
    pub update: UpdatePreferences,
    /// Legacy — migrated into `chrome` on load, not written on save.
    #[serde(default, skip_serializing, rename = "ui_font_size")]
    legacy_ui_font_size: f32,
    #[serde(default, skip_serializing, rename = "mono_font_size")]
    legacy_mono_font_size: f32,
    #[serde(default, skip_serializing, rename = "table_density")]
    legacy_table_density: LegacyTableDensity,
}

#[derive(Clone, Copy, Debug, Default, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
enum LegacyTableDensity {
    #[default]
    Compact,
    Comfortable,
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
            onboarding_completed: false,
            update: UpdatePreferences::default(),
            legacy_ui_font_size: 14.0,
            legacy_mono_font_size: 14.0,
            legacy_table_density: LegacyTableDensity::Compact,
        }
    }
}

impl Global for NativePreferences {}

fn migrate_legacy_theme_names(prefs: &mut NativePreferences) {
    if prefs.light_theme == "Default Light" {
        prefs.light_theme = crate::theme::DEFAULT_LIGHT_THEME.to_string();
    }
    if prefs.dark_theme == "Default Dark" {
        prefs.dark_theme = crate::theme::DEFAULT_DARK_THEME.to_string();
    }
}

fn migrate_legacy_chrome(prefs: &mut NativePreferences, raw: &str) {
    if raw.contains("[ui]") || raw.contains("density_preset") {
        prefs.chrome.sync_density_preset();
        return;
    }
    let table_compact = prefs.legacy_table_density == LegacyTableDensity::Compact;
    prefs.chrome.migrate_from_legacy(
        prefs.legacy_ui_font_size,
        prefs.legacy_mono_font_size,
        table_compact,
    );
}

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
        let raw = std::fs::read_to_string(&path).unwrap_or_default();
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
            if let Some(preset) = crate::theme::preset_by_id(preset_id) {
                prefs.light_theme = preset.light_name.to_string();
                prefs.dark_theme = preset.dark_name.to_string();
            }
        }
        migrate_legacy_theme_names(&mut prefs);
        migrate_legacy_chrome(&mut prefs, &raw);
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
        if let Err(e) = std::fs::write(path, encoded) {
            log::warn!("native prefs save: {e:#}");
        }
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
    if let Err(err) = crate::theme::apply_theme_names(&light, &dark, cx) {
        log::warn!("theme pair on startup: {err:#}");
        let _ = crate::theme::apply_theme_names(
            crate::theme::DEFAULT_LIGHT_THEME,
            crate::theme::DEFAULT_DARK_THEME,
            cx,
        );
    }
    reapply_appearance(None, cx);
    apply_chrome(&chrome, cx);
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

pub fn table_size_token(cx: &App) -> SizeToken {
    cx.global::<NativePreferences>().chrome.table.size
}

pub fn ui_component_size(cx: &App) -> Size {
    ui_size_token(cx).to_component_size()
}

pub fn page_size(cx: &App) -> u64 {
    cx.global::<NativePreferences>().page_size
}

pub fn query_timeout_secs(cx: &App) -> u32 {
    cx.global::<NativePreferences>().query_timeout_secs
}

pub fn table_prefs(cx: &App) -> TablePreferences {
    cx.global::<NativePreferences>().table_prefs
}

pub fn table_cell_size(cx: &App) -> Size {
    table_size_token(cx).to_component_size()
}

fn refresh_table_windows(changed: bool, cx: &mut App) {
    if changed {
        cx.refresh_windows();
    }
}

fn update_table_prefs(update: impl FnOnce(&mut TablePreferences) -> bool, cx: &mut App) {
    let changed = cx.update_global(|p: &mut NativePreferences, _| {
        let changed = update(&mut p.table_prefs);
        if changed {
            p.save_best_effort();
        }
        changed
    });
    refresh_table_windows(changed, cx);
}

pub fn set_table_stripe(stripe: bool, cx: &mut App) {
    update_table_prefs(
        |p| {
            if p.stripe == stripe {
                return false;
            }
            p.stripe = stripe;
            true
        },
        cx,
    );
}

pub fn set_table_sortable(sortable: bool, cx: &mut App) {
    update_table_prefs(
        |p| {
            if p.sortable == sortable {
                return false;
            }
            p.sortable = sortable;
            true
        },
        cx,
    );
}

pub fn set_table_col_resizable(col_resizable: bool, cx: &mut App) {
    update_table_prefs(
        |p| {
            if p.col_resizable == col_resizable {
                return false;
            }
            p.col_resizable = col_resizable;
            true
        },
        cx,
    );
}

pub fn set_table_col_movable(col_movable: bool, cx: &mut App) {
    update_table_prefs(
        |p| {
            if p.col_movable == col_movable {
                return false;
            }
            p.col_movable = col_movable;
            true
        },
        cx,
    );
}

pub fn set_table_row_selectable(row_selectable: bool, cx: &mut App) {
    update_table_prefs(
        |p| {
            if p.row_selectable == row_selectable {
                return false;
            }
            p.row_selectable = row_selectable;
            true
        },
        cx,
    );
}

pub fn set_table_cell_selectable(cell_selectable: bool, cx: &mut App) {
    update_table_prefs(
        |p| {
            if p.cell_selectable == cell_selectable {
                return false;
            }
            p.cell_selectable = cell_selectable;
            true
        },
        cx,
    );
}

pub fn set_table_loop_selection(loop_selection: bool, cx: &mut App) {
    update_table_prefs(
        |p| {
            if p.loop_selection == loop_selection {
                return false;
            }
            p.loop_selection = loop_selection;
            true
        },
        cx,
    );
}

pub fn set_table_size(size: SizeToken, cx: &mut App) {
    update_chrome(
        |c| {
            if c.table.size == size {
                return false;
            }
            c.table.size = size;
            true
        },
        cx,
    );
    cx.refresh_windows();
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

/// Switch light/dark appearance (legacy helper).
pub fn apply_theme(mode: ThemeMode, cx: &mut App) {
    let appearance = match mode {
        ThemeMode::Light => AppearanceMode::Light,
        ThemeMode::Dark => AppearanceMode::Dark,
    };
    apply_appearance(appearance, None, cx);
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

/// Preset id when light+dark match a known pair (onboarding selection state).
pub fn theme_preset_id(cx: &App) -> &str {
    crate::theme::preset_id_for_pair(light_theme_name(cx), dark_theme_name(cx))
        .unwrap_or(crate::theme::DEFAULT_PRESET_ID)
}

pub fn apply_appearance(mode: AppearanceMode, window: Option<&mut gpui::Window>, cx: &mut App) {
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

pub fn reapply_appearance(window: Option<&mut gpui::Window>, cx: &mut App) {
    let mode = appearance_mode(cx);
    apply_appearance(mode, window, cx);
}

/// Temporarily apply a light theme for dropdown preview (does not persist).
pub fn preview_light_theme(name: &str, window: Option<&mut gpui::Window>, cx: &mut App) {
    let dark = dark_theme_name(cx).to_string();
    if let Err(err) = crate::theme::apply_theme_names(name, &dark, cx) {
        log::warn!("preview light theme {name:?}: {err:#}");
        return;
    }
    reapply_appearance(window, cx);
}

/// Temporarily apply a dark theme for dropdown preview (does not persist).
pub fn preview_dark_theme(name: &str, window: Option<&mut gpui::Window>, cx: &mut App) {
    let light = light_theme_name(cx).to_string();
    if let Err(err) = crate::theme::apply_theme_names(&light, name, cx) {
        log::warn!("preview dark theme {name:?}: {err:#}");
        return;
    }
    reapply_appearance(window, cx);
}

pub fn revert_light_theme_preview(window: Option<&mut gpui::Window>, cx: &mut App) {
    let name = light_theme_name(cx).to_string();
    preview_light_theme(&name, window, cx);
}

pub fn revert_dark_theme_preview(window: Option<&mut gpui::Window>, cx: &mut App) {
    let name = dark_theme_name(cx).to_string();
    preview_dark_theme(&name, window, cx);
}

pub fn apply_theme_pair(light: &str, dark: &str, window: Option<&mut gpui::Window>, cx: &mut App) {
    if let Err(err) = crate::theme::apply_theme_names(light, dark, cx) {
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

pub fn apply_light_theme(name: &str, window: Option<&mut gpui::Window>, cx: &mut App) {
    let dark = dark_theme_name(cx).to_string();
    apply_theme_pair(name, &dark, window, cx);
}

pub fn apply_dark_theme(name: &str, window: Option<&mut gpui::Window>, cx: &mut App) {
    let light = light_theme_name(cx).to_string();
    apply_theme_pair(&light, name, window, cx);
}

/// Apply a paired preset (onboarding): sets both light and dark registry themes.
pub fn apply_theme_preset(preset_id: &str, window: Option<&mut gpui::Window>, cx: &mut App) {
    let Some(preset) = crate::theme::preset_by_id(preset_id) else {
        log::warn!("apply theme preset: unknown preset {preset_id:?}");
        return;
    };
    apply_theme_pair(preset.light_name, preset.dark_name, window, cx);
}

pub fn cycle_theme(cx: &mut App) {
    let next = match appearance_mode(cx) {
        AppearanceMode::Light => AppearanceMode::Dark,
        AppearanceMode::Dark => AppearanceMode::System,
        AppearanceMode::System => AppearanceMode::Light,
    };
    apply_appearance(next, None, cx);
}

pub fn onboarding_completed(cx: &App) -> bool {
    cx.global::<NativePreferences>().onboarding_completed
}

pub fn set_onboarding_completed(completed: bool, cx: &mut App) {
    cx.update_global(|p: &mut NativePreferences, _| {
        if p.onboarding_completed == completed {
            return;
        }
        p.onboarding_completed = completed;
        p.save_best_effort();
    });
}

pub fn update_prefs(cx: &App) -> UpdatePreferences {
    cx.global::<NativePreferences>().update.clone()
}

fn update_update_prefs(update: impl FnOnce(&mut UpdatePreferences) -> bool, cx: &mut App) {
    let changed = cx.update_global(|p: &mut NativePreferences, _| {
        let changed = update(&mut p.update);
        if changed {
            p.save_best_effort();
        }
        changed
    });
    if changed {
        cx.refresh_windows();
    }
}

pub fn set_update_check_at_startup(enabled: bool, cx: &mut App) {
    update_update_prefs(
        |u| {
            if u.check_at_startup == enabled {
                return false;
            }
            u.check_at_startup = enabled;
            true
        },
        cx,
    );
}

pub fn set_update_auto_check(enabled: bool, cx: &mut App) {
    update_update_prefs(
        |u| {
            if u.auto_check == enabled {
                return false;
            }
            u.auto_check = enabled;
            true
        },
        cx,
    );
}

pub fn set_update_check_interval(interval: UpdateCheckInterval, cx: &mut App) {
    update_update_prefs(
        |u| {
            if u.check_interval == interval {
                return false;
            }
            u.check_interval = interval;
            true
        },
        cx,
    );
}

pub fn set_update_auto_download(enabled: bool, cx: &mut App) {
    update_update_prefs(
        |u| {
            if u.auto_download == enabled {
                return false;
            }
            u.auto_download = enabled;
            true
        },
        cx,
    );
}

pub fn set_update_show_release_notes(enabled: bool, cx: &mut App) {
    update_update_prefs(
        |u| {
            if u.show_release_notes_after_update == enabled {
                return false;
            }
            u.show_release_notes_after_update = enabled;
            true
        },
        cx,
    );
}

pub fn set_update_include_prereleases(enabled: bool, cx: &mut App) {
    update_update_prefs(
        |u| {
            if u.include_prereleases == enabled {
                return false;
            }
            u.include_prereleases = enabled;
            true
        },
        cx,
    );
}
