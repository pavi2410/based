use gpui::{App, Entity, IntoElement, RenderOnce, SharedString, prelude::*, px};
use gpui_component::{
    ActiveTheme, Icon, Sizable as _,
    button::{Button, ButtonVariants},
    h_flex,
};

use crate::bindings::{ToggleHistoryPane, ToggleInspectorPane, ToggleSavedPane};
use crate::widgets::status_item::{STATUS_BAR_HEIGHT, status_divider, status_segment, status_text};
use crate::workspace::Workspace;
use crate::workspace::chrome::{left_pane::LeftPane, side_pane::SidePane};

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
    workspace: Entity<Workspace>,
    active_side_pane: Option<SidePane>,
    active_left_pane: LeftPane,
}

impl StatusBar {
    pub fn new(
        model: StatusBarModel,
        workspace: Entity<Workspace>,
        active_side_pane: Option<SidePane>,
        active_left_pane: LeftPane,
    ) -> Self {
        Self {
            model,
            workspace,
            active_side_pane,
            active_left_pane,
        }
    }
}

fn left_pane_button(
    pane: LeftPane,
    active: LeftPane,
    workspace: Entity<Workspace>,
    cx: &App,
) -> impl IntoElement {
    let is_active = active == pane;
    let color = if is_active {
        cx.theme().accent_foreground
    } else {
        cx.theme().muted_foreground
    };
    let id = match pane {
        LeftPane::Browser => "status-left-browser",
        LeftPane::Workspace => "status-left-workspace",
    };

    Button::new(id)
        .ghost()
        .small()
        .icon(Icon::new(pane.icon()).text_color(color))
        .tooltip(pane.tooltip())
        .on_click(move |_, _, cx| {
            workspace.update(cx, |w, cx| w.toggle_left_pane(pane, cx));
        })
}

fn side_pane_button(
    pane: SidePane,
    active: Option<SidePane>,
    workspace: Entity<Workspace>,
    cx: &App,
) -> impl IntoElement {
    let is_active = active == Some(pane);
    let color = if is_active {
        cx.theme().accent_foreground
    } else {
        cx.theme().muted_foreground
    };
    let id = match pane {
        SidePane::Inspector => "status-inspector",
        SidePane::History => "status-history",
        SidePane::Saved => "status-saved",
    };
    let (action, tooltip_text) = match pane {
        SidePane::Inspector => (&ToggleInspectorPane as &dyn gpui::Action, pane.tooltip()),
        SidePane::History => (&ToggleHistoryPane as &dyn gpui::Action, pane.tooltip()),
        SidePane::Saved => (&ToggleSavedPane as &dyn gpui::Action, pane.tooltip()),
    };

    Button::new(id)
        .ghost()
        .small()
        .icon(Icon::new(pane.icon()).text_color(color))
        .tooltip_with_action(tooltip_text, action, None)
        .on_click(move |_, _, cx| {
            workspace.update(cx, |w, cx| w.toggle_side_pane(pane, cx));
        })
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

        let workspace = self.workspace.clone();
        let active_side_pane = self.active_side_pane;
        let active_left_pane = self.active_left_pane;

        h_flex()
            .h(px(STATUS_BAR_HEIGHT))
            .w_full()
            .px(px(8.0))
            .border_t_1()
            .border_color(cx.theme().border)
            .bg(cx.theme().tab_bar)
            .items_center()
            .justify_between()
            .child(
                h_flex()
                    .gap(px(8.0))
                    .items_center()
                    .min_w_0()
                    .child(
                        h_flex()
                            .gap(px(1.0))
                            .items_center()
                            .flex_shrink_0()
                            .children(LeftPane::ALL.map(|pane| {
                                left_pane_button(pane, active_left_pane, workspace.clone(), cx)
                            })),
                    )
                    .child(status_divider(muted))
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
                    .gap(px(6.0))
                    .items_center()
                    .flex_shrink_0()
                    .child(status_segment(
                        "history",
                        history_value,
                        muted,
                        history_fg,
                        history_dot,
                    ))
                    .child(status_divider(muted))
                    .child(status_text("based 0.1.0", muted))
                    .child(status_divider(muted))
                    .child(
                        h_flex()
                            .gap(px(2.0))
                            .items_center()
                            .children(SidePane::ALL.map(|pane| {
                                side_pane_button(pane, active_side_pane, workspace.clone(), cx)
                            })),
                    ),
            )
    }
}
