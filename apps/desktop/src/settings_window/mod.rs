//! Separate settings window (theme, typography, query defaults).

use gpui::{
    App, Context, FocusHandle, Focusable, IntoElement, ParentElement, Render, SharedString,
    StyleRefinement, Styled, Window, div, prelude::*, px,
};
use gpui_component::{
    ActiveTheme, Icon, IconName, ThemeMode,
    group_box::GroupBoxVariant,
    searchable_list::SearchableListItem,
    setting::{NumberFieldOptions, SettingField, SettingGroup, SettingItem, SettingPage, Settings},
    v_flex,
};

use crate::app::prefs::{
    self, AppearanceMode, CodeFontFamilyId, DEFAULT_PAGE_SIZE, DEFAULT_QUERY_TIMEOUT_SECS,
    DensityPreset, FontWeightToken, NativePreferences, SizeToken, UiFontFamilyId,
};
use crate::theme::{DEFAULT_DARK_THEME, DEFAULT_LIGHT_THEME, ThemeNameItem};

#[cfg(target_os = "macos")]
fn macos_settings_header_style() -> StyleRefinement {
    StyleRefinement::default().pt(px(32.0))
}

fn size_token_options() -> Vec<(SharedString, SharedString)> {
    SizeToken::ALL
        .iter()
        .map(|token| (token.storage_key().into(), token.label().into()))
        .collect()
}

fn weight_token_options() -> Vec<(SharedString, SharedString)> {
    FontWeightToken::ALL
        .iter()
        .map(|token| (token.storage_key().into(), token.label().into()))
        .collect()
}

fn ui_font_options() -> Vec<(SharedString, SharedString)> {
    UiFontFamilyId::ALL
        .iter()
        .map(|id| (id.storage_key().into(), id.label().into()))
        .collect()
}

fn code_font_options() -> Vec<(SharedString, SharedString)> {
    CodeFontFamilyId::ALL
        .iter()
        .map(|id| (id.storage_key().into(), id.label().into()))
        .collect()
}

fn appearance_mode_options() -> Vec<(SharedString, SharedString)> {
    AppearanceMode::ALL
        .iter()
        .map(|mode| (mode.storage_key().into(), mode.label().into()))
        .collect()
}

fn density_preset_options() -> Vec<(SharedString, SharedString)> {
    let mut options: Vec<(SharedString, SharedString)> = DensityPreset::SELECTABLE
        .iter()
        .map(|preset| (preset.storage_key().into(), preset.label().into()))
        .collect();
    options.push((
        DensityPreset::Custom.storage_key().into(),
        DensityPreset::Custom.label().into(),
    ));
    options
}

fn theme_name_options(mode: ThemeMode, cx: &App) -> Vec<(SharedString, SharedString)> {
    ThemeNameItem::items_for_mode(mode, cx)
        .into_iter()
        .map(|item| {
            let name = item.value().clone();
            (name.clone(), item.title())
        })
        .collect()
}

pub struct SettingsWindow {
    focus_handle: FocusHandle,
}

impl SettingsWindow {
    pub fn new(cx: &mut Context<Self>) -> Self {
        Self {
            focus_handle: cx.focus_handle(),
        }
    }

    fn pages(&self, _: &mut Window, cx: &mut Context<Self>) -> Vec<SettingPage> {
        let light_theme_options = theme_name_options(ThemeMode::Light, cx);
        let dark_theme_options = theme_name_options(ThemeMode::Dark, cx);

        vec![
            SettingPage::new("Appearance")
                .default_open(true)
                .icon(Icon::new(IconName::Settings2))
                .groups(vec![
                    SettingGroup::new().title("Theme").items(vec![
                        SettingItem::new(
                            "Appearance",
                            SettingField::dropdown(
                                appearance_mode_options(),
                                |cx: &App| prefs::appearance_mode(cx).storage_key().into(),
                                |val: SharedString, cx: &mut App| {
                                    if let Some(mode) = AppearanceMode::from_storage_key(&val) {
                                        prefs::apply_appearance(mode, None, cx);
                                    }
                                },
                            )
                            .default_value(AppearanceMode::default().storage_key().to_string()),
                        )
                        .description("Light, dark, or match the system."),
                        SettingItem::new(
                            "Light theme",
                            SettingField::scrollable_dropdown(
                                light_theme_options,
                                |cx: &App| prefs::light_theme_name(cx).into(),
                                |val: SharedString, cx: &mut App| {
                                    prefs::apply_light_theme(val.as_ref(), None, cx);
                                },
                            )
                            .default_value(DEFAULT_LIGHT_THEME.to_string()),
                        )
                        .description("Theme used in light mode."),
                        SettingItem::new(
                            "Dark theme",
                            SettingField::scrollable_dropdown(
                                dark_theme_options,
                                |cx: &App| prefs::dark_theme_name(cx).into(),
                                |val: SharedString, cx: &mut App| {
                                    prefs::apply_dark_theme(val.as_ref(), None, cx);
                                },
                            )
                            .default_value(DEFAULT_DARK_THEME.to_string()),
                        )
                        .description("Theme used in dark mode."),
                    ]),
                    SettingGroup::new().title("Density").items(vec![SettingItem::new(
                        "UI density",
                        SettingField::dropdown(
                            density_preset_options(),
                            |cx: &App| prefs::density_preset(cx).storage_key().into(),
                            |val: SharedString, cx: &mut App| {
                                if let Some(preset) = DensityPreset::from_storage_key(&val) {
                                    prefs::apply_density_preset(preset, cx);
                                }
                            },
                        )
                        .default_value(DensityPreset::Default.storage_key().to_string()),
                    )
                    .description(
                        "Compact, default, or comfortable spacing for UI, editor, and tables.",
                    )]),
                    SettingGroup::new().title("UI font").items(vec![
                        SettingItem::new(
                            "Font family",
                            SettingField::dropdown(
                                ui_font_options(),
                                |cx: &App| {
                                    cx.global::<NativePreferences>()
                                        .chrome
                                        .ui
                                        .family
                                        .storage_key()
                                        .into()
                                },
                                |val: SharedString, cx: &mut App| {
                                    if let Some(family) = UiFontFamilyId::from_storage_key(&val) {
                                        prefs::set_ui_font_family(family, cx);
                                    }
                                },
                            )
                            .default_value(UiFontFamilyId::default().storage_key().to_string()),
                        )
                        .description("Interface font for sidebars, tabs, and controls."),
                        SettingItem::new(
                            "Font weight",
                            SettingField::dropdown(
                                weight_token_options(),
                                |cx: &App| {
                                    cx.global::<NativePreferences>()
                                        .chrome
                                        .ui
                                        .weight
                                        .storage_key()
                                        .into()
                                },
                                |val: SharedString, cx: &mut App| {
                                    if let Some(weight) = FontWeightToken::from_storage_key(&val) {
                                        prefs::set_ui_font_weight(weight, cx);
                                    }
                                },
                            )
                            .default_value(FontWeightToken::default().storage_key().to_string()),
                        ),
                        SettingItem::new(
                            "Size",
                            SettingField::dropdown(
                                size_token_options(),
                                |cx: &App| {
                                    cx.global::<NativePreferences>()
                                        .chrome
                                        .ui
                                        .size
                                        .storage_key()
                                        .into()
                                },
                                |val: SharedString, cx: &mut App| {
                                    if let Some(size) = SizeToken::from_storage_key(&val) {
                                        prefs::set_ui_size(size, cx);
                                    }
                                },
                            )
                            .default_value(SizeToken::default().storage_key().to_string()),
                        ),
                    ]),
                    SettingGroup::new().title("Query editor").items(vec![
                        SettingItem::new(
                            "Code font family",
                            SettingField::dropdown(
                                code_font_options(),
                                |cx: &App| {
                                    cx.global::<NativePreferences>()
                                        .chrome
                                        .editor
                                        .family
                                        .storage_key()
                                        .into()
                                },
                                |val: SharedString, cx: &mut App| {
                                    if let Some(family) = CodeFontFamilyId::from_storage_key(&val)
                                    {
                                        prefs::set_editor_font_family(family, cx);
                                    }
                                },
                            )
                            .default_value(CodeFontFamilyId::default().storage_key().to_string()),
                        )
                        .description("Monospace font for SQL and JSON editors."),
                        SettingItem::new(
                            "Font weight",
                            SettingField::dropdown(
                                weight_token_options(),
                                |cx: &App| {
                                    cx.global::<NativePreferences>()
                                        .chrome
                                        .editor
                                        .weight
                                        .storage_key()
                                        .into()
                                },
                                |val: SharedString, cx: &mut App| {
                                    if let Some(weight) = FontWeightToken::from_storage_key(&val) {
                                        prefs::set_editor_font_weight(weight, cx);
                                    }
                                },
                            )
                            .default_value(FontWeightToken::default().storage_key().to_string()),
                        ),
                        SettingItem::new(
                            "Size",
                            SettingField::dropdown(
                                size_token_options(),
                                |cx: &App| {
                                    cx.global::<NativePreferences>()
                                        .chrome
                                        .editor
                                        .size
                                        .storage_key()
                                        .into()
                                },
                                |val: SharedString, cx: &mut App| {
                                    if let Some(size) = SizeToken::from_storage_key(&val) {
                                        prefs::set_editor_size(size, cx);
                                    }
                                },
                            )
                            .default_value(SizeToken::default().storage_key().to_string()),
                        ),
                    ]),
                    SettingGroup::new().title("Tables").items(vec![
                        SettingItem::new(
                            "Row size",
                            SettingField::dropdown(
                                size_token_options(),
                                |cx: &App| {
                                    cx.global::<NativePreferences>()
                                        .chrome
                                        .table
                                        .size
                                        .storage_key()
                                        .into()
                                },
                                |val: SharedString, cx: &mut App| {
                                    if let Some(size) = SizeToken::from_storage_key(&val) {
                                        prefs::set_table_size(size, cx);
                                    }
                                },
                            )
                            .default_value(SizeToken::XSmall.storage_key().to_string()),
                        )
                        .description("Row height and cell padding in data grids."),
                        SettingItem::new(
                            "Zebra striping",
                            SettingField::switch(
                                |cx| prefs::table_prefs(cx).stripe,
                                prefs::set_table_stripe,
                            ),
                        )
                        .description("Alternate row shading in data grids."),
                    ]),
                ]),
            SettingPage::new("Query defaults")
                .icon(Icon::new(IconName::Settings))
                .groups(vec![
                    SettingGroup::new().title("Data fetching").items(vec![
                        SettingItem::new(
                            "Page size",
                            SettingField::number_input(
                                NumberFieldOptions {
                                    min: 50.0,
                                    max: 5000.0,
                                    step: 50.0,
                                },
                                |cx| prefs::page_size(cx) as f64,
                                |val, cx| prefs::set_page_size(val as u64, cx),
                            )
                            .default_value(DEFAULT_PAGE_SIZE as f64),
                        )
                        .description("Rows fetched per page in SQL table viewers."),
                        SettingItem::new(
                            "Query timeout",
                            SettingField::number_input(
                                NumberFieldOptions {
                                    min: 5.0,
                                    max: 600.0,
                                    step: 5.0,
                                },
                                |cx| prefs::query_timeout_secs(cx) as f64,
                                |val, cx| prefs::set_query_timeout_secs(val as u32, cx),
                            )
                            .default_value(DEFAULT_QUERY_TIMEOUT_SECS as f64),
                        )
                        .description(
                            "Maximum seconds to wait for a query (enforcement in editors is planned).",
                        ),
                    ]),
                    SettingGroup::new().title("Table interaction").items(vec![
                        SettingItem::new(
                            "Sortable columns",
                            SettingField::switch(
                                |cx| prefs::table_prefs(cx).sortable,
                                prefs::set_table_sortable,
                            ),
                        )
                        .description("Click column headers to sort results."),
                        SettingItem::new(
                            "Resize columns",
                            SettingField::switch(
                                |cx| prefs::table_prefs(cx).col_resizable,
                                prefs::set_table_col_resizable,
                            ),
                        )
                        .description("Drag column borders to change width."),
                        SettingItem::new(
                            "Reorder columns",
                            SettingField::switch(
                                |cx| prefs::table_prefs(cx).col_movable,
                                prefs::set_table_col_movable,
                            ),
                        )
                        .description("Drag column headers to rearrange."),
                        SettingItem::new(
                            "Row selection",
                            SettingField::switch(
                                |cx| prefs::table_prefs(cx).row_selectable,
                                prefs::set_table_row_selectable,
                            ),
                        )
                        .description("Select entire rows in data grids."),
                        SettingItem::new(
                            "Cell selection",
                            SettingField::switch(
                                |cx| prefs::table_prefs(cx).cell_selectable,
                                prefs::set_table_cell_selectable,
                            ),
                        )
                        .description("Select individual cells; enables keyboard navigation."),
                        SettingItem::new(
                            "Loop selection",
                            SettingField::switch(
                                |cx| prefs::table_prefs(cx).loop_selection,
                                prefs::set_table_loop_selection,
                            ),
                        )
                        .description("Keyboard selection wraps at table edges."),
                    ]),
                ]),
        ]
    }
}

impl Focusable for SettingsWindow {
    fn focus_handle(&self, _: &gpui::App) -> FocusHandle {
        self.focus_handle.clone()
    }
}

impl Render for SettingsWindow {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let bg = cx.theme().background;
        let fg = cx.theme().foreground;

        let mut settings = Settings::new("based-settings")
            .with_group_variant(GroupBoxVariant::Outline)
            .pages(self.pages(window, cx));
        #[cfg(target_os = "macos")]
        {
            settings = settings.header_style(&macos_settings_header_style());
        }

        v_flex()
            .size_full()
            .bg(bg)
            .text_color(fg)
            .when(!cfg!(target_os = "macos"), |this| this.pt(px(36.0)))
            .child(
                div()
                    .flex_1()
                    .min_h_0()
                    .size_full()
                    .bg(bg)
                    .text_color(fg)
                    .child(settings),
            )
    }
}
