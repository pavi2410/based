//! First-run onboarding and Help → review window (theme + shortcuts).
//!
//! First launch opens this window **before** the main workspace. Finish Setup or
//! closing the window completes onboarding and opens the workspace with a Welcome tab.
//! Help → Onboarding reopens the same UI as a non-blocking aux window.

use gpui::{
    App, Context, FocusHandle, Focusable, FontWeight, IntoElement, ParentElement, Render, Styled,
    Window, div, prelude::FluentBuilder, px,
};
use gpui_component::{
    ActiveTheme, StyledExt,
    button::{Button, ButtonVariants},
    h_flex,
    kbd::Kbd,
    scroll::ScrollableElement,
    v_flex,
};

use crate::app::launch;
use crate::app::shell::OpenSettingsMenu;
use crate::bindings::{
    ToggleCommandPalette, ToggleHistoryPane, ToggleInspectorPane, ToggleSidebarRail,
};
use crate::widgets::kbd_for_action;

const ONBOARDING_COLUMN_W: f32 = 560.0;

/// First-run gate vs Help-menu review (same content; only the gate completes onboarding on close).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum OnboardingMode {
    FirstRunGate,
    Review,
}

pub struct OnboardingWindow {
    focus_handle: FocusHandle,
    mode: OnboardingMode,
}

impl OnboardingWindow {
    pub fn new(mode: OnboardingMode, cx: &mut Context<Self>) -> Self {
        Self {
            focus_handle: cx.focus_handle(),
            mode,
        }
    }

    pub fn mode(&self) -> OnboardingMode {
        self.mode
    }
}

impl Focusable for OnboardingWindow {
    fn focus_handle(&self, _: &App) -> FocusHandle {
        self.focus_handle.clone()
    }
}

impl Render for OnboardingWindow {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let show_finish = self.mode == OnboardingMode::FirstRunGate;
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
                            .child(onboarding_header(show_finish, cx))
                            .child(crate::theme::theme_onboarding_picker("onboarding", cx))
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

fn finish_setup(window: &mut Window, cx: &mut App) {
    launch::complete_onboarding(cx);
    launch::AppLaunch::clear_gate(cx);
    window.remove_window();
}

fn onboarding_header(show_finish: bool, cx: &mut App) -> impl IntoElement {
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
        .when(show_finish, |row| {
            row.child(
                Button::new("finish-setup")
                    .primary()
                    .label("Finish Setup")
                    .on_click(|_, window, cx| finish_setup(window, cx)),
            )
        })
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
