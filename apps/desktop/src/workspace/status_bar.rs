use gpui::{App, IntoElement, RenderOnce, SharedString, prelude::*, px};
use gpui_component::{ActiveTheme, h_flex};

use crate::widgets::status_item::{status_divider, status_segment, status_text};

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

        let history_value = if self.model.history_ready {
            "ready"
        } else {
            "empty"
        };
        let history_fg = if self.model.history_ready {
            fg
        } else {
            cx.theme().warning_foreground
        };
        let history_dot = if self.model.history_ready {
            Some(cx.theme().green_light.opacity(0.85))
        } else {
            None
        };

        h_flex()
            .h(px(22.0))
            .w_full()
            .px_3()
            .border_t_1()
            .border_color(cx.theme().border)
            .bg(cx.theme().tab_bar)
            .items_center()
            .justify_between()
            .child(
                h_flex()
                    .gap(px(10.0))
                    .items_center()
                    .child(status_segment(
                        "connections",
                        self.model.connection_count.to_string(),
                        muted,
                        fg,
                        None,
                    ))
                    .child(status_divider(muted))
                    .child(status_segment(
                        "live",
                        self.model.connected_count.to_string(),
                        muted,
                        fg,
                        live_dot,
                    ))
                    .child(status_divider(muted))
                    .child(status_segment(
                        "scope",
                        self.model.scope_label,
                        muted,
                        fg,
                        None,
                    )),
            )
            .child(
                h_flex()
                    .gap(px(10.0))
                    .items_center()
                    .child(status_segment(
                        "history",
                        history_value,
                        muted,
                        history_fg,
                        history_dot,
                    ))
                    .child(status_divider(muted))
                    .child(status_text("based 0.1.0", muted)),
            )
    }
}
