use gpui::{App, IntoElement, RenderOnce, div, prelude::*};
use gpui_component::{ActiveTheme, h_flex};

/// A thin status bar rendered at the bottom of the workspace.
#[derive(IntoElement)]
pub struct StatusBar {
    pub connection_count: usize,
}

impl StatusBar {
    pub fn new(connection_count: usize) -> Self {
        Self { connection_count }
    }
}

impl RenderOnce for StatusBar {
    fn render(self, _window: &mut gpui::Window, cx: &mut App) -> impl IntoElement {
        h_flex()
            .h_7()
            .w_full()
            .px_3()
            .border_t_1()
            .border_color(cx.theme().border)
            .bg(cx.theme().tab_bar)
            .items_center()
            .justify_between()
            .child(
                div()
                    .text_xs()
                    .text_color(cx.theme().muted_foreground)
                    .child(format!("{} connections", self.connection_count)),
            )
            .child(
                div()
                    .text_xs()
                    .text_color(cx.theme().muted_foreground)
                    .child("based v0.1.0"),
            )
    }
}
