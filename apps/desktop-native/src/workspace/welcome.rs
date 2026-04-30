use gpui::{App, Context, FocusHandle, Focusable, IntoElement, Render, Window, div, prelude::*};
use gpui_component::{
    ActiveTheme, StyledExt,
    dock::{Panel, PanelEvent},
    h_flex, v_flex,
};

pub struct WelcomePanel {
    focus_handle: FocusHandle,
}

impl WelcomePanel {
    pub fn new(_window: &mut Window, cx: &mut Context<Self>) -> Self {
        Self {
            focus_handle: cx.focus_handle(),
        }
    }
}

impl gpui::EventEmitter<PanelEvent> for WelcomePanel {}

impl Focusable for WelcomePanel {
    fn focus_handle(&self, _: &App) -> FocusHandle {
        self.focus_handle.clone()
    }
}

impl Panel for WelcomePanel {
    fn panel_name(&self) -> &'static str {
        "WelcomePanel"
    }

    fn closable(&self, _: &App) -> bool {
        false
    }
}

impl Render for WelcomePanel {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        v_flex()
            .size_full()
            .items_center()
            .justify_center()
            .gap_8()
            .bg(cx.theme().background)
            .child(
                v_flex()
                    .items_center()
                    .gap_2()
                    .child(
                        div()
                            .text_3xl()
                            .font_bold()
                            .text_color(cx.theme().foreground)
                            .child("based"),
                    )
                    .child(
                        div()
                            .text_sm()
                            .text_color(cx.theme().muted_foreground)
                            .child("Git-Friendly Database Client"),
                    ),
            )
            .child(
                h_flex()
                    .gap_4()
                    .child(action_card(cx, "Open Project", "Open an existing project folder"))
                    .child(action_card(cx, "New Connection", "Add a new database connection"))
                    .child(action_card(cx, "Recent", "Open a recently used connection")),
            )
    }
}

fn action_card(
    cx: &mut Context<WelcomePanel>,
    title: &'static str,
    subtitle: &'static str,
) -> impl IntoElement {
    div()
        .w_48()
        .p_4()
        .rounded_lg()
        .border_1()
        .border_color(cx.theme().border)
        .bg(cx.theme().background)
        .cursor_pointer()
        .hover(|s| s.border_color(gpui::hsla(0.0, 0.0, 0.5, 1.0)))
        .child(
            v_flex()
                .gap_1()
                .child(
                    div()
                        .text_sm()
                        .font_bold()
                        .text_color(cx.theme().foreground)
                        .child(title),
                )
                .child(
                    div()
                        .text_xs()
                        .text_color(cx.theme().muted_foreground)
                        .child(subtitle),
                ),
        )
}
