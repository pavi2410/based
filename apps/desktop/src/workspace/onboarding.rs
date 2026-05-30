use gpui::{
    App, Context, FocusHandle, Focusable, FontWeight, InteractiveElement, IntoElement, MouseButton,
    ParentElement, Render, SharedString, Styled, Window, div, px,
};
use gpui_component::{
    ActiveTheme, Icon, IconName, Sizable as _, StyledExt,
    button::{Button, ButtonVariants},
    dock::{Panel, PanelEvent},
    h_flex,
    kbd::Kbd,
    menu::PopupMenu,
    scroll::ScrollableElement,
    v_flex,
};

use crate::app::prefs;
use crate::app::shell::OpenSettingsMenu;
use crate::bindings::{
    ToggleCommandPalette, ToggleHistoryPane, ToggleInspectorPane, ToggleSidebarRail,
};
use crate::widgets::ui::kbd_for_action;
use crate::workspace::query_lane::create_loose_query_from_palette;
use crate::workspace::tab_open::{enqueue_open_postgres_wizard, enqueue_show_welcome};

const ONBOARDING_COLUMN_W: f32 = 560.0;

pub struct OnboardingPanel {
    focus_handle: FocusHandle,
    pub(crate) tab_label: SharedString,
}

impl OnboardingPanel {
    pub fn new(_window: &mut Window, cx: &mut Context<Self>) -> Self {
        Self {
            focus_handle: cx.focus_handle(),
            tab_label: "Onboarding".into(),
        }
    }
}

impl gpui::EventEmitter<PanelEvent> for OnboardingPanel {}

impl Focusable for OnboardingPanel {
    fn focus_handle(&self, _: &App) -> FocusHandle {
        self.focus_handle.clone()
    }
}

impl Panel for OnboardingPanel {
    fn panel_name(&self) -> &'static str {
        "OnboardingPanel"
    }

    fn dropdown_menu(
        &mut self,
        menu: PopupMenu,
        _window: &mut Window,
        cx: &mut Context<Self>,
    ) -> PopupMenu {
        crate::based_panel_dropdown!(menu, self, cx)
    }

    crate::based_panel_tab_chrome!();
}

impl Render for OnboardingPanel {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        v_flex()
            .size_full()
            .items_center()
            .justify_start()
            .overflow_y_scrollbar()
            .bg(cx.theme().background)
            .child(
                v_flex()
                    .w_full()
                    .items_center()
                    .py(px(32.0))
                    .px(px(24.0))
                    .child(
                        v_flex()
                            .w(px(ONBOARDING_COLUMN_W))
                            .gap(px(32.0))
                            .child(onboarding_header(cx))
                            .child(crate::theme::theme_onboarding_picker("onboarding", cx))
                            .child(onboarding_section(
                                "Get Started",
                                "Connect to a database or open a query tab.",
                                cx,
                                v_flex()
                                    .gap(px(8.0))
                                    .child(action_row(
                                        cx,
                                        "onboarding-new-connection",
                                        IconName::Globe,
                                        "New Connection",
                                        |_, _, cx| {
                                            enqueue_open_postgres_wizard(cx);
                                            cx.refresh_windows();
                                        },
                                    ))
                                    .child(action_row(
                                        cx,
                                        "onboarding-new-query",
                                        IconName::Plus,
                                        "New Query",
                                        |_, _, cx| create_loose_query_from_palette(cx),
                                    )),
                            ))
                            .child(onboarding_section(
                                "Keyboard Shortcuts",
                                "Common shortcuts to get around the workspace.",
                                cx,
                                shortcuts_grid(window, cx),
                            )),
                    ),
            )
    }
}

fn finish_setup(_window: &mut Window, cx: &mut App) {
    prefs::set_onboarding_completed(true, cx);
    enqueue_show_welcome(cx);
    cx.refresh_windows();
}

fn onboarding_header(cx: &mut App) -> impl IntoElement {
    let fg = cx.theme().foreground;
    let muted = cx.theme().muted_foreground;

    h_flex()
        .w_full()
        .items_start()
        .justify_between()
        .gap(px(16.0))
        .child(
            v_flex()
                .gap(px(6.0))
                .child(div().text_2xl().font_bold().text_color(fg).child("Based"))
                .child(
                    div()
                        .text_lg()
                        .font_bold()
                        .text_color(fg)
                        .child("Welcome to Based"),
                )
                .child(
                    div()
                        .text_sm()
                        .italic()
                        .text_color(muted)
                        .child("Git-Friendly Database Client"),
                ),
        )
        .child(
            Button::new("finish-setup")
                .primary()
                .label("Finish Setup")
                .on_click(|_, window, cx| finish_setup(window, cx)),
        )
}

fn onboarding_section(
    title: &'static str,
    description: &'static str,
    cx: &App,
    body: impl IntoElement,
) -> impl IntoElement {
    let muted = cx.theme().muted_foreground;
    v_flex()
        .gap(px(12.0))
        .child(
            v_flex()
                .gap(px(4.0))
                .child(section_title(title, cx))
                .child(div().text_sm().text_color(muted).child(description)),
        )
        .child(body)
}

fn section_title(label: &str, cx: &App) -> impl IntoElement {
    div()
        .text_sm()
        .font_weight(FontWeight::BOLD)
        .text_color(cx.theme().foreground)
        .child(label.to_string())
}

fn action_row(
    cx: &App,
    id: &'static str,
    icon: IconName,
    label: &'static str,
    on_click: impl Fn(&gpui::MouseDownEvent, &mut Window, &mut App) + 'static,
) -> impl IntoElement {
    let fg = cx.theme().foreground;
    let muted = cx.theme().muted_foreground;
    let hover_bg = cx.theme().muted.opacity(0.35);

    h_flex()
        .id(id)
        .w_full()
        .h(px(36.0))
        .px(px(10.0))
        .rounded(px(6.0))
        .items_center()
        .gap(px(10.0))
        .cursor_pointer()
        .hover(move |s| s.bg(hover_bg))
        .on_mouse_down(MouseButton::Left, on_click)
        .child(
            Icon::new(icon)
                .text_color(muted)
                .with_size(gpui_component::Size::Small),
        )
        .child(div().flex_1().text_sm().text_color(fg).child(label))
}

fn shortcuts_grid(window: &Window, cx: &App) -> impl IntoElement {
    let rows: [(&str, Option<Kbd>); 5] = [
        (
            "Command palette",
            kbd_for_action(&ToggleCommandPalette, window),
        ),
        ("Settings", kbd_for_action(&OpenSettingsMenu, window)),
        ("Toggle sidebar", kbd_for_action(&ToggleSidebarRail, window)),
        (
            "Inspector pane",
            kbd_for_action(&ToggleInspectorPane, window),
        ),
        ("History pane", kbd_for_action(&ToggleHistoryPane, window)),
    ];

    v_flex().gap(px(6.0)).children(
        rows.into_iter()
            .map(|(label, kbd)| shortcut_row(label, kbd, cx)),
    )
}

fn shortcut_row(label: &'static str, kbd: Option<Kbd>, cx: &App) -> impl IntoElement {
    let fg = cx.theme().foreground;
    let muted = cx.theme().muted_foreground;
    h_flex()
        .w_full()
        .items_center()
        .justify_between()
        .py(px(4.0))
        .child(div().text_sm().text_color(fg).child(label))
        .children(kbd.into_iter().map(|k| div().text_color(muted).child(k)))
}
