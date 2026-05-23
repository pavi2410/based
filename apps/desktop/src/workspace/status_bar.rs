use gpui::{App, IntoElement, RenderOnce, SharedString, prelude::*};
use gpui_component::{ActiveTheme, h_flex};

use crate::widgets::status_chip::status_chip;

/// Context passed into the workspace status rail.
#[derive(Clone, Debug)]
pub struct StatusBarModel {
    pub connection_count: usize,
    pub connected_count: usize,
    pub scope_label: SharedString,
    pub history_ready: bool,
}

/// A thin status bar rendered at the bottom of the workspace.
#[derive(IntoElement)]
pub struct StatusBar {
    model: StatusBarModel,
}

impl StatusBar {
    pub fn new(model: StatusBarModel) -> Self {
        Self { model }
    }
}

impl RenderOnce for StatusBar {
    fn render(self, _window: &mut gpui::Window, cx: &mut App) -> impl IntoElement {
        let muted = cx.theme().muted_foreground;
        let fg = cx.theme().foreground;
        let live_dot = if self.model.connected_count > 0 {
            Some(cx.theme().green_light)
        } else {
            None
        };
        let history_dot = if self.model.history_ready {
            Some(cx.theme().green_light.opacity(0.85))
        } else {
            Some(muted.opacity(0.5))
        };

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
                    .gap_2()
                    .items_center()
                    .child(status_chip(
                        "connections",
                        self.model.connection_count.to_string(),
                        muted,
                        fg,
                        None,
                    ))
                    .child(status_chip(
                        "live",
                        self.model.connected_count.to_string(),
                        muted,
                        fg,
                        live_dot,
                    ))
                    .child(status_chip(
                        "scope",
                        self.model.scope_label,
                        muted,
                        fg,
                        None,
                    )),
            )
            .child(
                h_flex()
                    .gap_2()
                    .items_center()
                    .child(status_chip(
                        "history",
                        SharedString::from(if self.model.history_ready {
                            "ready"
                        } else {
                            "empty"
                        }),
                        muted,
                        fg,
                        history_dot,
                    ))
                    .child(status_chip(
                        "based",
                        "v0.1.0",
                        muted,
                        fg,
                        None,
                    )),
            )
    }
}
