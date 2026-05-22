//! Separate settings window (theme, typography, query defaults).

use gpui::{Context, FocusHandle, Focusable, IntoElement, Render, Window, prelude::*};
use gpui_component::{
    ActiveTheme, Icon, IconName, Theme, ThemeMode,
    group_box::GroupBoxVariant,
    setting::{NumberFieldOptions, SettingField, SettingGroup, SettingItem, SettingPage, Settings},
};

use crate::app::prefs::{self, DEFAULT_PAGE_SIZE, DEFAULT_QUERY_TIMEOUT_SECS, NativePreferences};

pub struct SettingsWindow {
    focus_handle: FocusHandle,
}

impl SettingsWindow {
    pub fn new(cx: &mut Context<Self>) -> Self {
        Self {
            focus_handle: cx.focus_handle(),
        }
    }

    fn pages(&self, _: &mut Window, _cx: &mut Context<Self>) -> Vec<SettingPage> {
        vec![
            SettingPage::new("Appearance")
                .default_open(true)
                .icon(Icon::new(IconName::Settings2))
                .groups(vec![
                    SettingGroup::new().title("Theme").items(vec![
                        SettingItem::new(
                            "Dark mode",
                            SettingField::switch(
                                |cx| cx.theme().mode.is_dark(),
                                |enabled, cx| {
                                    let mode = if enabled {
                                        ThemeMode::Dark
                                    } else {
                                        ThemeMode::Light
                                    };
                                    Theme::change(mode, None, cx);
                                    cx.update_global(|p: &mut NativePreferences, _| {
                                        p.theme_mode = mode;
                                        p.save_best_effort();
                                    });
                                    prefs::apply_font_sizes(cx);
                                },
                            ),
                        )
                        .description("Switch between light and dark themes."),
                    ]),
                    SettingGroup::new().title("Typography").items(vec![
                        SettingItem::new(
                            "UI font size",
                            SettingField::number_input(
                                NumberFieldOptions {
                                    min: 10.0,
                                    max: 24.0,
                                    step: 1.0,
                                    ..Default::default()
                                },
                                |cx| prefs::ui_font_size(cx) as f64,
                                |val, cx| prefs::set_ui_font_size(val as f32, cx),
                            )
                            .default_value(prefs::DEFAULT_UI_FONT_SIZE as f64),
                        )
                        .description("Base UI font size in pixels."),
                        SettingItem::new(
                            "Monospace font size",
                            SettingField::number_input(
                                NumberFieldOptions {
                                    min: 10.0,
                                    max: 22.0,
                                    step: 1.0,
                                    ..Default::default()
                                },
                                |cx| prefs::mono_font_size(cx) as f64,
                                |val, cx| prefs::set_mono_font_size(val as f32, cx),
                            )
                            .default_value(prefs::DEFAULT_MONO_FONT_SIZE as f64),
                        )
                        .description("Monospace font size for editors and SQL."),
                    ]),
                ]),
            SettingPage::new("Query defaults")
                .icon(Icon::new(IconName::Settings))
                .groups(vec![SettingGroup::new().title("Data browsing").items(
                    vec![
                    SettingItem::new(
                        "Page size",
                        SettingField::number_input(
                            NumberFieldOptions {
                                min: 50.0,
                                max: 5000.0,
                                step: 50.0,
                                ..Default::default()
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
                                ..Default::default()
                            },
                            |cx| prefs::query_timeout_secs(cx) as f64,
                            |val, cx| prefs::set_query_timeout_secs(val as u32, cx),
                        )
                        .default_value(DEFAULT_QUERY_TIMEOUT_SECS as f64),
                    )
                    .description(
                        "Maximum seconds to wait for a query (enforcement in editors is planned).",
                    ),
                ],
                )]),
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
        Settings::new("based-settings")
            .with_group_variant(GroupBoxVariant::Outline)
            .pages(self.pages(window, cx))
    }
}
