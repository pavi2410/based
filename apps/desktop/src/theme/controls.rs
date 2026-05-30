//! Shared appearance + theme controls (onboarding, settings).

use gpui::{
    App, Entity, Hsla, InteractiveElement, IntoElement, ParentElement, SharedString, Styled,
    Window, div, prelude::FluentBuilder, px,
};
use gpui_component::{
    ActiveTheme, ThemeConfig, ThemeMode, ThemeRegistry,
    button::{Toggle, ToggleGroup, ToggleVariants},
    h_flex,
    searchable_list::SearchableListItem,
    select::{Select, SelectState},
    v_flex,
};

use crate::app::prefs::{self, AppearanceMode};

use super::presets::{self, ThemePreset};

/// Which settings dropdown axis a preview session tracks.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ThemePreviewAxis {
    Light,
    Dark,
}

/// Tracks a non-persisted theme preview while browsing a settings dropdown.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ThemePreviewSession {
    pub axis: ThemePreviewAxis,
    active: bool,
    last_name: Option<String>,
}

impl ThemePreviewSession {
    pub fn new(axis: ThemePreviewAxis) -> Self {
        Self {
            axis,
            active: false,
            last_name: None,
        }
    }

    pub fn preview(&mut self, name: &str, window: Option<&mut Window>, cx: &mut App) {
        let committed = match self.axis {
            ThemePreviewAxis::Light => prefs::light_theme_name(cx),
            ThemePreviewAxis::Dark => prefs::dark_theme_name(cx),
        };
        if name == committed {
            if self.active {
                self.revert(window, cx);
            }
            return;
        }
        if self.last_name.as_deref() == Some(name) {
            return;
        }
        match self.axis {
            ThemePreviewAxis::Light => prefs::preview_light_theme(name, window, cx),
            ThemePreviewAxis::Dark => prefs::preview_dark_theme(name, window, cx),
        }
        self.active = true;
        self.last_name = Some(name.to_string());
    }

    pub fn revert(&mut self, window: Option<&mut Window>, cx: &mut App) {
        if !self.active {
            return;
        }
        match self.axis {
            ThemePreviewAxis::Light => prefs::revert_light_theme_preview(window, cx),
            ThemePreviewAxis::Dark => prefs::revert_dark_theme_preview(window, cx),
        }
        self.active = false;
        self.last_name = None;
    }

    pub fn clear_after_commit(&mut self) {
        self.active = false;
        self.last_name = None;
    }

    pub fn is_active(&self) -> bool {
        self.active
    }
}

/// Registry theme name row for settings dropdowns.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ThemeNameItem {
    name: SharedString,
}

impl ThemeNameItem {
    pub fn new(name: impl Into<SharedString>) -> Self {
        Self { name: name.into() }
    }

    pub fn items_for_mode(mode: ThemeMode, cx: &App) -> Vec<Self> {
        ThemeRegistry::global(cx)
            .sorted_themes()
            .into_iter()
            .filter(|theme| theme.mode == mode)
            .filter(|theme| !is_hidden_registry_theme(theme.name.as_ref()))
            .map(|theme| Self::new(theme.name.clone()))
            .collect()
    }
}

fn is_hidden_registry_theme(name: &str) -> bool {
    matches!(name, "Default Light" | "Default Dark")
}

impl SearchableListItem for ThemeNameItem {
    type Value = SharedString;

    fn title(&self) -> SharedString {
        self.name.clone()
    }

    fn value(&self) -> &Self::Value {
        &self.name
    }
}

const BUNDLE_THEMES: &[&str] = &[
    include_str!("bundles/based.json"),
    include_str!("bundles/gruvbox.json"),
    include_str!("bundles/ayu.json"),
    include_str!("bundles/catppuccin.json"),
    include_str!("bundles/everforest.json"),
    include_str!("bundles/solarized.json"),
    include_str!("bundles/github_high_contrast.json"),
    include_str!("bundles/github_colorblind.json"),
];

/// Load bundled third-party theme JSON into the registry (idempotent).
pub fn load_bundled_themes(cx: &mut App) {
    let registry = ThemeRegistry::global_mut(cx);
    for content in BUNDLE_THEMES {
        if let Err(err) = registry.load_themes_from_str(content) {
            log::warn!("theme bundle load: {err:#}");
        }
    }
}

/// Segmented Light / Dark / System control.
pub fn appearance_segmented(id_prefix: &'static str, mode: AppearanceMode) -> impl IntoElement {
    let checks = [
        mode == AppearanceMode::Light,
        mode == AppearanceMode::Dark,
        mode == AppearanceMode::System,
    ];

    ToggleGroup::new(format!("{id_prefix}-appearance"))
        .segmented()
        .outline()
        .child(Toggle::new(0).label("Light").checked(checks[0]))
        .child(Toggle::new(1).label("Dark").checked(checks[1]))
        .child(Toggle::new(2).label("System").checked(checks[2]))
        .on_click(move |states, window, cx| {
            let mode = if states.first() == Some(&true) {
                AppearanceMode::Light
            } else if states.get(1) == Some(&true) {
                AppearanceMode::Dark
            } else {
                AppearanceMode::System
            };
            prefs::apply_appearance(mode, Some(window), cx);
        })
}

/// Theme name picker for settings (dropdown).
pub fn theme_name_select(select: Entity<SelectState<Vec<ThemeNameItem>>>) -> impl IntoElement {
    Select::new(&select).w_full()
}

/// Zed-style onboarding theme section: header + segmented control + 3 preview cards.
pub fn theme_onboarding_picker(id_prefix: &'static str, cx: &App) -> impl IntoElement {
    let active_preset = prefs::theme_preset_id(cx);
    let appearance = prefs::appearance_mode(cx);
    let border = cx.theme().border;
    let accent_fg = cx.theme().accent_foreground;
    let label_fg = cx.theme().foreground;

    v_flex()
        .w_full()
        .gap(px(16.0))
        .child(
            h_flex()
                .w_full()
                .items_center()
                .justify_between()
                .gap(px(12.0))
                .child(
                    div()
                        .text_sm()
                        .font_weight(gpui::FontWeight::SEMIBOLD)
                        .child("Theme"),
                )
                .child(appearance_segmented(id_prefix, appearance)),
        )
        .child(
            h_flex()
                .w_full()
                .gap(px(12.0))
                .children(presets::onboarding_presets().map(|preset| {
                    let theme_name = preview_theme_name_for_preset(preset, appearance);
                    let preview_theme = theme_name.and_then(|name| theme_config_for_name(name, cx));
                    onboarding_preset_card(
                        format!("{id_prefix}-preset-{}", preset.id),
                        preset,
                        preset.id == active_preset,
                        preview_theme,
                        border,
                        accent_fg,
                        label_fg,
                        move |_, window, cx| {
                            prefs::apply_theme_preset(preset.id, Some(window), cx);
                        },
                    )
                })),
        )
}

fn onboarding_preset_card(
    id: impl Into<gpui::ElementId>,
    preset: &ThemePreset,
    selected: bool,
    preview_theme: Option<ThemeConfig>,
    border: Hsla,
    accent_fg: Hsla,
    label_fg: Hsla,
    on_click: impl Fn(&gpui::MouseDownEvent, &mut Window, &mut App) + 'static,
) -> impl IntoElement {
    let mut card = v_flex()
        .id(id.into())
        .flex_1()
        .min_w_0()
        .gap(px(8.0))
        .cursor_pointer()
        .on_mouse_down(gpui::MouseButton::Left, on_click);

    if selected {
        card = card.border_2().border_color(accent_fg);
    } else {
        card = card.border_1().border_color(border);
    }

    card.rounded(px(8.0))
        .p(px(8.0))
        .child(
            div()
                .w_full()
                .h(px(120.0))
                .rounded(px(6.0))
                .overflow_hidden()
                .when_some(preview_theme.as_ref(), |this, theme| {
                    this.child(theme_table_preview(theme))
                }),
        )
        .child(
            div()
                .text_sm()
                .text_center()
                .text_color(if selected { accent_fg } else { label_fg })
                .child(preset.label),
        )
}

fn preview_theme_name_for_preset(
    preset: &ThemePreset,
    appearance: AppearanceMode,
) -> Option<&'static str> {
    match appearance {
        AppearanceMode::Light | AppearanceMode::System => Some(preset.light_name),
        AppearanceMode::Dark => Some(preset.dark_name),
    }
}

fn theme_config_for_name(name: &str, cx: &App) -> Option<ThemeConfig> {
    ThemeRegistry::global(cx)
        .themes()
        .get(&SharedString::from(name))
        .map(|theme| theme.as_ref().clone())
}

fn theme_table_preview(theme: &ThemeConfig) -> impl IntoElement + use<> {
    let colors = &theme.colors;
    let bg = optional_color(colors.background.as_ref()).unwrap_or_else(|| parse_color("#ffffff"));
    let fg = optional_color(colors.foreground.as_ref()).unwrap_or_else(|| parse_color("#1a1d23"));
    let border = optional_color(colors.border.as_ref()).unwrap_or_else(|| parse_color("#d9dce2"));
    let header_bg = optional_color(colors.muted.as_ref()).unwrap_or(bg);
    let row_alt = optional_color(colors.list_even.as_ref()).unwrap_or(header_bg);
    let sidebar = optional_color(colors.sidebar.as_ref()).unwrap_or(header_bg);

    h_flex()
        .size_full()
        .bg(bg)
        .child(
            div()
                .w(px(28.0))
                .h_full()
                .bg(sidebar)
                .border_r_1()
                .border_color(border),
        )
        .child(
            v_flex()
                .flex_1()
                .h_full()
                .child(
                    h_flex()
                        .w_full()
                        .h(px(18.0))
                        .bg(header_bg)
                        .border_b_1()
                        .border_color(border)
                        .children(
                            (0..3)
                                .map(|_| div().flex_1().h_full().border_r_1().border_color(border)),
                        ),
                )
                .children((0..4).map(|row| {
                    let row_bg = if row % 2 == 1 { row_alt } else { bg };
                    h_flex()
                        .w_full()
                        .h(px(16.0))
                        .bg(row_bg)
                        .border_b_1()
                        .border_color(border.opacity(0.6))
                        .children((0..3).map(|col| {
                            let w = match (row, col) {
                                (0, 0) => px(32.0),
                                (1, 1) => px(40.0),
                                (2, 2) => px(28.0),
                                _ => px(36.0),
                            };
                            div()
                                .my_auto()
                                .ml(px(6.0))
                                .w(w)
                                .h(px(6.0))
                                .rounded(px(2.0))
                                .bg(fg.opacity(0.25))
                        }))
                })),
        )
}

fn optional_color(value: Option<&SharedString>) -> Option<Hsla> {
    value.map(|hex| parse_color(hex))
}

fn parse_color(hex: &str) -> Hsla {
    gpui_component::try_parse_color(hex).unwrap_or(gpui::Hsla::white())
}
