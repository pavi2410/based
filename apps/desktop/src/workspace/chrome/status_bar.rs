use gpui::{App, IntoElement, RenderOnce, SharedString, prelude::*, px};
use gpui_component::{
    ActiveTheme, Icon, IconName, Sizable as _,
    button::{Button, ButtonVariants},
    h_flex,
    menu::{DropdownMenu, PopupMenuItem},
};

use crate::app::updater::UpdateBarSnapshot;
use crate::app::updater::{self, UpdatePhase, is_dev_build};
use crate::bindings::{ToggleHistoryPane, ToggleInspectorPane, ToggleSavedPane};
use crate::connection::registry::ConnectionRegistry;
use crate::widgets::status_item::{STATUS_BAR_HEIGHT, status_divider, status_segment, status_text};
use crate::workspace::chrome::{left_pane::LeftPane, side_pane::SidePane};
use crate::workspace::tab_open::{
    WorkspaceRef, enqueue_toggle_left_pane, enqueue_toggle_side_pane,
};

/// Context passed into the workspace status rail.
#[derive(Clone, Debug)]
pub struct StatusBarModel {
    pub connection_count: usize,
    pub connected_count: usize,
    pub scope_label: SharedString,
    pub history_ready: bool,
    pub update: UpdateBarSnapshot,
}

/// A thin status bar rendered at the bottom of the workspace.
#[derive(IntoElement)]
pub struct StatusBar {
    model: StatusBarModel,
    active_side_pane: Option<SidePane>,
    active_left_pane: LeftPane,
    registry: gpui::Entity<ConnectionRegistry>,
}

impl StatusBar {
    pub fn new(
        model: StatusBarModel,
        active_side_pane: Option<SidePane>,
        active_left_pane: LeftPane,
        registry: gpui::Entity<ConnectionRegistry>,
    ) -> Self {
        Self {
            model,
            active_side_pane,
            active_left_pane,
            registry,
        }
    }
}

fn left_pane_button(pane: LeftPane, active: LeftPane, cx: &App) -> impl IntoElement {
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
            enqueue_toggle_left_pane(pane, cx);
            if let Some(ws) = cx.try_global::<WorkspaceRef>().map(|w| w.0.clone()) {
                ws.update(cx, |_, cx| cx.notify());
            }
        })
}

fn side_pane_button(pane: SidePane, active: Option<SidePane>, cx: &App) -> impl IntoElement {
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
            enqueue_toggle_side_pane(pane, cx);
            if let Some(ws) = cx.try_global::<WorkspaceRef>().map(|w| w.0.clone()) {
                ws.update(cx, |_, cx| cx.notify());
            }
        })
}

fn update_widget(
    snapshot: &UpdateBarSnapshot,
    registry: gpui::Entity<ConnectionRegistry>,
    cx: &App,
) -> Option<impl IntoElement> {
    if is_dev_build() {
        return None;
    }

    let phase = snapshot.phase;
    if matches!(phase, UpdatePhase::Idle) {
        return None;
    }

    let muted = cx.theme().muted_foreground;
    let accent = cx.theme().accent_foreground;
    let warning = cx.theme().warning_foreground;
    let danger = cx.theme().danger_foreground;

    let (dot, fg, label) = match phase {
        UpdatePhase::Checking => (None, muted, SharedString::from("Checking…")),
        UpdatePhase::UpToDate => (None, muted, SharedString::from("Up to date")),
        UpdatePhase::Available => (
            Some(accent),
            accent,
            snapshot
                .version
                .as_ref()
                .map(|v| SharedString::from(format!("Update {v}")))
                .unwrap_or_else(|| SharedString::from("Update available")),
        ),
        UpdatePhase::Downloading => {
            let pct = snapshot.progress_percent;
            (
                Some(accent),
                accent,
                SharedString::from(format!("Downloading… {pct}%")),
            )
        }
        UpdatePhase::Ready => (
            Some(warning),
            warning,
            SharedString::from("Restart to update"),
        ),
        UpdatePhase::Failed => (Some(danger), danger, SharedString::from("Update failed")),
        UpdatePhase::Idle => unreachable!(),
    };

    let has_version = snapshot.version.is_some();

    Some(
        Button::new("status-update")
            .ghost()
            .small()
            .child(status_segment("update", label, muted, fg, dot))
            .dropdown_menu(move |menu, _window, _cx| {
                let registry = registry.clone();
                let mut menu = menu;
                if matches!(
                    phase,
                    UpdatePhase::Available | UpdatePhase::Ready | UpdatePhase::Failed
                ) {
                    menu = menu.item(
                        PopupMenuItem::new("Check for updates")
                            .icon(IconName::Inbox)
                            .on_click(|_, _, cx| updater::check_now(cx)),
                    );
                }
                if matches!(phase, UpdatePhase::Available) {
                    menu = menu.item(
                        PopupMenuItem::new("Download update")
                            .icon(IconName::Plus)
                            .on_click(|_, _, cx| updater::start_download(cx)),
                    );
                }
                if matches!(phase, UpdatePhase::Ready | UpdatePhase::Available) {
                    menu = menu.item(
                        PopupMenuItem::new("Install & Restart")
                            .icon(IconName::Play)
                            .on_click(move |_, window, cx| {
                                updater::install_and_restart(&registry, window, cx);
                            }),
                    );
                }
                if has_version {
                    menu = menu.item(
                        PopupMenuItem::new("Release notes")
                            .icon(IconName::BookOpen)
                            .on_click(|_, _, cx| updater::open_release_notes_for_current(cx)),
                    );
                }
                if matches!(phase, UpdatePhase::Available) {
                    menu = menu.item(PopupMenuItem::separator()).item(
                        PopupMenuItem::new("Later").on_click(|_, _, cx| updater::dismiss(cx)),
                    );
                }
                if matches!(phase, UpdatePhase::Failed) {
                    menu = menu.item(
                        PopupMenuItem::new("Open releases page")
                            .icon(IconName::ExternalLink)
                            .on_click(|_, _, cx| updater::open_releases_page(cx)),
                    );
                }
                menu
            }),
    )
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

        let active_side_pane = self.active_side_pane;
        let active_left_pane = self.active_left_pane;
        let version_label = format!("based {}", env!("CARGO_PKG_VERSION"));
        let registry = self.registry.clone();
        let update_snapshot = self.model.update.clone();

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
                            .children(
                                LeftPane::ALL
                                    .map(|pane| left_pane_button(pane, active_left_pane, cx)),
                            ),
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
                    .when_some(
                        update_widget(&update_snapshot, registry, cx),
                        |row, widget| row.child(widget).child(status_divider(muted)),
                    )
                    .child(status_text(version_label, muted))
                    .child(status_divider(muted))
                    .child(h_flex().gap(px(2.0)).items_center().children(
                        SidePane::ALL.map(|pane| side_pane_button(pane, active_side_pane, cx)),
                    )),
            )
    }
}
