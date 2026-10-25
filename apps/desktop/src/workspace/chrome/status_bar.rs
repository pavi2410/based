use gpui_kit::component::{
    ActiveTheme, Icon, IconName, Sizable as _,
    button::{Button, ButtonVariants},
    h_flex,
    menu::{DropdownMenu, PopupMenuItem},
};
use gpui_kit::{App, Entity, IntoElement, RenderOnce, SharedString, div, prelude::*, px};

use crate::ai::AiStatus;
use crate::app::prefs::{self, update_check_settings_locked};
use crate::app::shell::open_settings;
use crate::app::updater::UpdateBarSnapshot;
use crate::app::updater::{self, UpdatePhase};
use crate::bindings::{ToggleHistoryPane, ToggleInspectorPane, ToggleSavedPane};
use crate::connection::registry::ConnectionRegistry;
use crate::connection::{ConnectionEntry, ConnectionId, ConnectionState, EngineKind};
use crate::widgets::engine_icon;
use crate::widgets::status_item::{STATUS_BAR_HEIGHT, status_divider, status_text};
use crate::workspace::ConnectionTree;
use crate::workspace::chrome::side_pane::SidePane;
use crate::workspace::connection_tree::connection_actions_menu;
use crate::workspace::tabs::{WorkspaceRef, enqueue_toggle_side_pane};

/// Focused connection shown as a status-bar chip.
#[derive(Clone, Debug)]
pub struct StatusBarConnection {
    pub id: ConnectionId,
    pub label: SharedString,
    pub engine: EngineKind,
    pub is_connected: bool,
    pub state_color: gpui_kit::Hsla,
    pub muted: bool,
}

impl StatusBarConnection {
    pub fn from_entry(entry: &ConnectionEntry, muted: bool, cx: &App) -> Self {
        Self {
            id: entry.id.clone(),
            label: entry.config.label().to_string().into(),
            engine: entry.config.engine(),
            is_connected: matches!(entry.state, ConnectionState::Connected(_)),
            state_color: match &entry.state {
                ConnectionState::Disconnected => cx.theme().muted_foreground.opacity(0.75),
                ConnectionState::Connecting => cx.theme().warning_foreground,
                ConnectionState::Connected(_) => cx.theme().green_light,
                ConnectionState::Failed { .. } => cx.theme().danger_foreground,
            },
            muted,
        }
    }
}

/// Context passed into the workspace status rail.
#[derive(Clone, Debug)]
pub struct StatusBarModel {
    pub update: UpdateBarSnapshot,
    pub focused_connection: Option<StatusBarConnection>,
}

/// A thin status bar rendered at the bottom of the workspace.
#[derive(IntoElement)]
pub struct StatusBar {
    model: StatusBarModel,
    active_side_pane: Option<SidePane>,
    registry: gpui_kit::Entity<ConnectionRegistry>,
    connection_tree: Entity<ConnectionTree>,
}

impl StatusBar {
    pub fn new(
        model: StatusBarModel,
        active_side_pane: Option<SidePane>,
        registry: gpui_kit::Entity<ConnectionRegistry>,
        connection_tree: Entity<ConnectionTree>,
    ) -> Self {
        Self {
            model,
            active_side_pane,
            registry,
            connection_tree,
        }
    }
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
        SidePane::Inspector => (
            &ToggleInspectorPane as &dyn gpui_kit::Action,
            pane.tooltip(),
        ),
        SidePane::History => (&ToggleHistoryPane as &dyn gpui_kit::Action, pane.tooltip()),
        SidePane::Saved => (&ToggleSavedPane as &dyn gpui_kit::Action, pane.tooltip()),
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

fn connection_chip(
    chip: StatusBarConnection,
    tree: Entity<ConnectionTree>,
    cx: &App,
) -> impl IntoElement {
    let muted = cx.theme().muted_foreground;
    let fg = if chip.muted {
        muted
    } else {
        cx.theme().foreground
    };
    let label = chip.label.clone();
    let engine = chip.engine;
    let is_connected = chip.is_connected;
    let conn_id = chip.id.clone();
    let tree = tree.downgrade();

    Button::new("status-connection")
        .ghost()
        .small()
        .child(
            h_flex()
                .items_center()
                .gap(px(6.0))
                .max_w(px(180.0))
                .child(engine_icon(engine))
                .child(
                    div()
                        .min_w_0()
                        .flex_1()
                        .text_xs()
                        .text_color(fg)
                        .truncate()
                        .child(label),
                )
                .child(
                    div()
                        .w(px(6.0))
                        .h(px(6.0))
                        .rounded_full()
                        .flex_shrink_0()
                        .bg(chip.state_color),
                ),
        )
        .dropdown_menu(move |menu, _window, _cx| {
            connection_actions_menu(
                conn_id.clone(),
                engine,
                is_connected,
                tree.clone(),
                menu,
                false,
            )
        })
}

/// Model in use and AI activity; click opens Settings, where provider and keys live.
fn ai_item(cx: &App) -> impl IntoElement {
    let status = AiStatus::get(cx);
    let ai = prefs::ai_prefs(cx);
    let provider = ai.provider();
    let model: SharedString = ai.model().to_string().into();
    let streaming = status.streaming();
    let tokens = status.tokens();
    let color = if streaming {
        cx.theme().accent_foreground
    } else {
        cx.theme().muted_foreground
    };
    let tooltip = match tokens {
        0 => format!("AI: {} · {model}", provider.label()),
        n => format!(
            "AI: {} · {model} · {n} tokens this session",
            provider.label()
        ),
    };

    Button::new("status-ai")
        .ghost()
        .small()
        .tooltip(tooltip)
        .on_click(|_, _, cx| open_settings(cx))
        .child(
            h_flex()
                .items_center()
                .gap(px(4.0))
                .child(Icon::new(IconName::Bot).small().text_color(color))
                .child(
                    div()
                        .max_w(px(140.0))
                        .truncate()
                        .text_xs()
                        .text_color(color)
                        .child(model),
                )
                .when(streaming, |row| {
                    row.child(
                        div()
                            .w(px(5.0))
                            .h(px(5.0))
                            .rounded_full()
                            .flex_shrink_0()
                            .bg(color),
                    )
                }),
        )
}

/// Version label doubling as the update entry point; the click menu depends on the phase.
fn version_item(
    snapshot: &UpdateBarSnapshot,
    registry: gpui_kit::Entity<ConnectionRegistry>,
    cx: &App,
) -> impl IntoElement {
    let phase = snapshot.phase;
    let locked = update_check_settings_locked();
    let can_install = snapshot.can_install;
    let muted = cx.theme().muted_foreground;
    let accent = cx.theme().accent_foreground;
    let warning = cx.theme().warning_foreground;
    let danger = cx.theme().danger_foreground;
    let current = env!("CARGO_PKG_VERSION");
    let new_version: SharedString = snapshot
        .version
        .as_ref()
        .map(|v| v.trim_start_matches('v').to_string())
        .unwrap_or_default()
        .into();

    let (suffix, color, tooltip): (Option<String>, _, String) = if locked {
        (None, muted, format!("Based {current}"))
    } else {
        match phase {
            UpdatePhase::Idle => (None, muted, format!("Based {current} — click for options")),
            UpdatePhase::Checking => (
                Some("checking…".into()),
                muted,
                "Checking for updates…".into(),
            ),
            UpdatePhase::UpToDate => (None, muted, format!("Based {current} is up to date")),
            UpdatePhase::Available => (
                Some(format!("→ {new_version}")),
                accent,
                format!("Based {new_version} is available — click to review"),
            ),
            UpdatePhase::Downloading => (
                Some(format!("{}%", snapshot.progress_percent)),
                accent,
                format!("Downloading {new_version}… {}%", snapshot.progress_percent),
            ),
            UpdatePhase::Ready => (
                Some("restart".into()),
                warning,
                format!("Based {new_version} is downloaded — click to install"),
            ),
            UpdatePhase::Failed => (
                Some("update failed".into()),
                danger,
                snapshot
                    .error
                    .as_ref()
                    .map(|e| e.to_string())
                    .unwrap_or_else(|| "Update failed".into()),
            ),
        }
    };
    let show_dot = matches!(
        phase,
        UpdatePhase::Available
            | UpdatePhase::Downloading
            | UpdatePhase::Ready
            | UpdatePhase::Failed
    ) && !locked;

    Button::new("status-version")
        .ghost()
        .small()
        .tooltip(tooltip)
        .child(
            h_flex()
                .items_center()
                .gap(px(4.0))
                .child(status_text(format!("based {current}"), muted))
                .when_some(suffix, |row, text| {
                    row.child(div().text_xs().text_color(color).child(text))
                })
                .when(show_dot, |row| {
                    row.child(
                        div()
                            .w(px(5.0))
                            .h(px(5.0))
                            .rounded_full()
                            .flex_shrink_0()
                            .bg(color),
                    )
                }),
        )
        .dropdown_menu(move |menu, _window, _cx| {
            let registry = registry.clone();
            let mut menu = menu;
            let notes_new = || {
                PopupMenuItem::new(format!("What's new in {new_version}"))
                    .icon(IconName::BookOpen)
                    .on_click(|_, _, cx| updater::open_release_notes_for_available(cx))
            };
            if !locked {
                match phase {
                    UpdatePhase::Available => {
                        menu = menu.item(notes_new());
                        menu = if can_install {
                            menu.item(
                                PopupMenuItem::new("Download update")
                                    .icon(IconName::ArrowDown)
                                    .on_click(|_, _, cx| updater::start_download(cx)),
                            )
                        } else {
                            menu.item(
                                PopupMenuItem::new("Open releases page")
                                    .icon(IconName::ExternalLink)
                                    .on_click(|_, _, cx| updater::open_releases_page(cx)),
                            )
                        };
                        menu = menu.item(
                            PopupMenuItem::new("Skip this version")
                                .icon(IconName::Close)
                                .on_click(|_, _, cx| updater::skip_version(cx)),
                        );
                        return menu;
                    }
                    UpdatePhase::Downloading => return menu.item(notes_new()),
                    UpdatePhase::Ready => {
                        return menu
                            .item(
                                PopupMenuItem::new("Install & Restart…")
                                    .icon(IconName::Play)
                                    .on_click(move |_, window, cx| {
                                        updater::install_and_restart(&registry, window, cx);
                                    }),
                            )
                            .item(notes_new());
                    }
                    _ => {
                        menu = menu.item(
                            PopupMenuItem::new("Check for updates")
                                .icon(IconName::Inbox)
                                .disabled(phase == UpdatePhase::Checking)
                                .on_click(|_, _, cx| updater::check_now(cx)),
                        );
                        if phase == UpdatePhase::Failed {
                            menu = menu.item(
                                PopupMenuItem::new("Open releases page")
                                    .icon(IconName::ExternalLink)
                                    .on_click(|_, _, cx| updater::open_releases_page(cx)),
                            );
                        }
                    }
                }
            }
            menu.item(
                PopupMenuItem::new("Release notes")
                    .icon(IconName::BookOpen)
                    .on_click(|_, _, cx| updater::open_release_notes_for_current(cx)),
            )
        })
}

impl RenderOnce for StatusBar {
    fn render(self, _window: &mut gpui_kit::Window, cx: &mut App) -> impl IntoElement {
        let muted = cx.theme().muted_foreground;

        let active_side_pane = self.active_side_pane;
        let registry = self.registry.clone();
        let update_snapshot = self.model.update.clone();
        let connection_tree = self.connection_tree.clone();
        let focused_connection = self.model.focused_connection.clone();

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
                    .when_some(focused_connection, |row, chip| {
                        row.child(connection_chip(chip, connection_tree, cx))
                    }),
            )
            .child(
                h_flex()
                    .gap(px(6.0))
                    .items_center()
                    .flex_shrink_0()
                    .child(ai_item(cx))
                    .child(status_divider(muted))
                    .child(version_item(&update_snapshot, registry, cx))
                    .child(status_divider(muted))
                    .child(h_flex().gap(px(2.0)).items_center().children(
                        SidePane::ALL.map(|pane| side_pane_button(pane, active_side_pane, cx)),
                    )),
            )
    }
}
