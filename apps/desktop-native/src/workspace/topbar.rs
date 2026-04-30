use gpui::{App, IntoElement, RenderOnce, SharedString, div, prelude::*};
use gpui_component::{
    ActiveTheme, IconName, StyledExt, TitleBar,
    button::{Button, ButtonVariants},
    h_flex,
};

/// A `RenderOnce` top bar that renders inside the window's `TitleBar`.
#[derive(IntoElement)]
pub struct Topbar {
    pub project_name: SharedString,
}

impl Topbar {
    pub fn new(project_name: impl Into<SharedString>) -> Self {
        Self {
            project_name: project_name.into(),
        }
    }
}

impl RenderOnce for Topbar {
    fn render(self, _window: &mut gpui::Window, cx: &mut App) -> impl IntoElement {
        TitleBar::new().child(
            h_flex()
                .w_full()
                .items_center()
                .justify_between()
                .child(
                    div()
                        .text_xs()
                        .text_color(cx.theme().muted_foreground)
                        .child("based"),
                )
                .child(
                    div()
                        .text_sm()
                        .font_semibold()
                        .text_color(cx.theme().foreground)
                        .child(self.project_name),
                )
                .child(
                    Button::new("settings")
                        .ghost()
                        .icon(IconName::Settings)
                        .on_click(|_, _, _| eprintln!("settings — Phase 2 will wire this")),
                ),
        )
    }
}
