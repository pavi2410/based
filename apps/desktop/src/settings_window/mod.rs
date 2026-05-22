//! Separate settings window (theme, typography, defaults).

use gpui::{Context, FontWeight, IntoElement, Render, Window, div, prelude::*};
use gpui_component::{
    ActiveTheme,
    button::Button,
    h_flex, v_flex,
};

pub struct SettingsWindow {
    pub page_size: u32,
    pub query_timeout_secs: u32,
}

impl SettingsWindow {
    pub fn new() -> Self {
        Self {
            page_size: 100,
            query_timeout_secs: 30,
        }
    }
}

impl Render for SettingsWindow {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let ui_font = crate::app::prefs::ui_font_size(cx);
        let mono_font = crate::app::prefs::mono_font_size(cx);

        v_flex()
            .size_full()
            .p_6()
            .gap_6()
            .bg(cx.theme().background)
            .child(
                div()
                    .text_lg()
                    .font_weight(FontWeight::SEMIBOLD)
                    .text_color(cx.theme().foreground)
                    .child("Settings"),
            )
            .child(
                v_flex()
                    .gap_4()
                    .child(
                        h_flex()
                            .gap_4()
                            .items_center()
                            .child(
                                div()
                                    .w(gpui::px(160.0))
                                    .text_sm()
                                    .text_color(cx.theme().foreground)
                                    .child("Theme"),
                            )
                            .child(
                                Button::new("theme-toggle")
                                    .label("Toggle Dark / Light")
                                    .on_click(cx.listener(|_, _, _, cx| {
                                        crate::app::prefs::cycle_theme(cx);
                                        cx.notify();
                                    })),
                            ),
                    )
                    .child(
                        h_flex()
                            .gap_4()
                            .items_center()
                            .child(
                                div()
                                    .w(gpui::px(160.0))
                                    .text_sm()
                                    .text_color(cx.theme().foreground)
                                    .child("UI font size"),
                            )
                            .child(
                                h_flex()
                                    .gap_2()
                                    .items_center()
                                    .child(
                                        Button::new("ui-font-dec")
                                            .label("−")
                                            .on_click(cx.listener(|_, _, _, cx| {
                                                crate::app::prefs::adjust_ui_font_size(-1.0, cx);
                                                cx.notify();
                                            })),
                                    )
                                    .child(
                                        div()
                                            .w(gpui::px(48.0))
                                            .text_sm()
                                            .text_center()
                                            .text_color(cx.theme().foreground)
                                            .child(format!("{ui_font:.0}px")),
                                    )
                                    .child(
                                        Button::new("ui-font-inc")
                                            .label("+")
                                            .on_click(cx.listener(|_, _, _, cx| {
                                                crate::app::prefs::adjust_ui_font_size(1.0, cx);
                                                cx.notify();
                                            })),
                                    ),
                            ),
                    )
                    .child(
                        h_flex()
                            .gap_4()
                            .items_center()
                            .child(
                                div()
                                    .w(gpui::px(160.0))
                                    .text_sm()
                                    .text_color(cx.theme().foreground)
                                    .child("Monospace font size"),
                            )
                            .child(
                                h_flex()
                                    .gap_2()
                                    .items_center()
                                    .child(
                                        Button::new("mono-font-dec")
                                            .label("−")
                                            .on_click(cx.listener(|_, _, _, cx| {
                                                crate::app::prefs::adjust_mono_font_size(-1.0, cx);
                                                cx.notify();
                                            })),
                                    )
                                    .child(
                                        div()
                                            .w(gpui::px(48.0))
                                            .text_sm()
                                            .text_center()
                                            .text_color(cx.theme().foreground)
                                            .child(format!("{mono_font:.0}px")),
                                    )
                                    .child(
                                        Button::new("mono-font-inc")
                                            .label("+")
                                            .on_click(cx.listener(|_, _, _, cx| {
                                                crate::app::prefs::adjust_mono_font_size(1.0, cx);
                                                cx.notify();
                                            })),
                                    ),
                            ),
                    )
                    .child(
                        div()
                            .text_xs()
                            .text_color(cx.theme().muted_foreground)
                            .child(format!(
                                "Shipped theme default: {}px UI, {}px mono (library default 16/13).",
                                crate::app::prefs::DEFAULT_UI_FONT_SIZE,
                                crate::app::prefs::DEFAULT_MONO_FONT_SIZE,
                            )),
                    )
                    .child(
                        h_flex()
                            .gap_4()
                            .items_center()
                            .child(
                                div()
                                    .w(gpui::px(160.0))
                                    .text_sm()
                                    .text_color(cx.theme().foreground)
                                    .child("Default page size"),
                            )
                            .child(
                                div()
                                    .text_sm()
                                    .text_color(cx.theme().muted_foreground)
                                    .child(format!("{} rows", self.page_size)),
                            ),
                    )
                    .child(
                        h_flex()
                            .gap_4()
                            .items_center()
                            .child(
                                div()
                                    .w(gpui::px(160.0))
                                    .text_sm()
                                    .text_color(cx.theme().foreground)
                                    .child("Query timeout"),
                            )
                            .child(
                                div()
                                    .text_sm()
                                    .text_color(cx.theme().muted_foreground)
                                    .child(format!("{}s", self.query_timeout_secs)),
                            ),
                    ),
            )
    }
}
