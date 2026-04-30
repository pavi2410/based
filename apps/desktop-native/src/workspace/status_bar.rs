use gpui::{App, IntoElement, RenderOnce, div, prelude::*};
use gpui_component::{ActiveTheme, h_flex};

/// A thin status bar rendered at the bottom of the workspace.
#[derive(IntoElement)]
pub struct StatusBar {
    pub connection_count: usize,
    pub connected_count: usize,
}

impl StatusBar {
    pub fn new(connection_count: usize, connected_count: usize) -> Self {
        Self {
            connection_count,
            connected_count,
        }
    }
}

impl RenderOnce for StatusBar {
    fn render(self, _window: &mut gpui::Window, cx: &mut App) -> impl IntoElement {
        h_flex()
            .h_6()
            .w_full()
            .px_2()
            .border_t_1()
            .border_color(cx.theme().border)
            .bg(cx.theme().tab_bar)
            .items_center()
            .justify_between()
            .child(
                h_flex()
                    .gap_3()
                    .items_center()
                    .child(
                        div()
                            .text_xs()
                            .font_family(cx.theme().mono_font_family.clone())
                            .text_color(cx.theme().muted_foreground)
                            .child(format!(
                                "{} connections · {} live",
                                self.connection_count, self.connected_count
                            )),
                    )
                    .child(
                        div()
                            .text_xs()
                            .text_color(cx.theme().muted_foreground.opacity(0.82))
                            .child("work locally"),
                    ),
            )
            .child(
                h_flex()
                    .gap_3()
                    .items_center()
                    .child(
                        div()
                            .text_xs()
                            .text_color(cx.theme().muted_foreground.opacity(0.82))
                            .child("query history ready"),
                    )
                    .child(
                        div()
                            .text_xs()
                            .font_family(cx.theme().mono_font_family.clone())
                            .text_color(cx.theme().muted_foreground)
                            .child("based v0.1.0"),
                    ),
            )
    }
}
