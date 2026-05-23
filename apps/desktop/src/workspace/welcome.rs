use gpui::{
    App, Context, FocusHandle, Focusable, IntoElement, Render, Window, div, prelude::*, px,
};
use gpui_component::{
    ActiveTheme, IconName, Sizable as _, StyledExt,
    dock::{Panel, PanelEvent},
    h_flex,
    menu::PopupMenu,
    v_flex,
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

    fn dropdown_menu(
        &mut self,
        menu: PopupMenu,
        _window: &mut Window,
        cx: &mut Context<Self>,
    ) -> PopupMenu {
        crate::based_panel_dropdown!(menu, self, cx)
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
            .gap_6()
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
                            .child("Graphite-native database workspace"),
                    ),
            )
            .child(
                div()
                    .text_xs()
                    .text_color(cx.theme().muted_foreground.opacity(0.9))
                    .child(if cfg!(target_os = "macos") {
                        "⌘K · local-first · SQLite · Postgres · MongoDB"
                    } else {
                        "Ctrl K · local-first · SQLite · Postgres · MongoDB"
                    }),
            )
            .child(
                h_flex()
                    .gap_3()
                    .child(action_card(
                        cx,
                        "Open Project",
                        "Open an existing project folder",
                        IconName::FolderOpen,
                    ))
                    .child(action_card(
                        cx,
                        "New Connection",
                        "Add a database connection",
                        IconName::Plus,
                    ))
                    .child(action_card(
                        cx,
                        "Command Center",
                        "Open anything from one place",
                        IconName::Search,
                    )),
            )
    }
}

fn action_card(
    cx: &mut Context<WelcomePanel>,
    title: &'static str,
    subtitle: &'static str,
    icon: IconName,
) -> impl IntoElement {
    div()
        .w(px(210.0))
        .p_3()
        .rounded(px(6.0))
        .cursor_pointer()
        .hover(|s| s.bg(cx.theme().muted.opacity(0.35)))
        .child(
            v_flex()
                .gap_2()
                .child(
                    gpui_component::Icon::new(icon)
                        .text_color(cx.theme().muted_foreground)
                        .with_size(gpui_component::Size::Small),
                )
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
