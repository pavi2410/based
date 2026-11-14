use gpui_kit::component::{
    ActiveTheme, Icon, IconName, Sizable as _,
    button::{Button, ButtonVariants},
    h_flex,
    menu::{DropdownMenu, PopupMenuItem},
};
use gpui_kit::{App, IntoElement, RenderOnce, SharedString, div, prelude::*, px};

use crate::ai::AiStatus;
use crate::app::prefs::{self, update_check_settings_locked};
use crate::app::shell::open_settings;
use crate::app::updater::{self, UpdateStatus};
use crate::bindings::{ToggleChatPane, ToggleHistoryPane, ToggleInspectorPane, ToggleSavedPane};
use crate::connection::registry::ConnectionRegistry;
use crate::widgets::status_item::{STATUS_BAR_HEIGHT, status_divider, status_text};
use crate::workspace::chrome::side_pane::SidePane;
use crate::workspace::tabs::{Workspaces, inbox};

/// Context passed into the workspace status rail.
#[derive(Clone, Debug)]
pub struct StatusBarModel {
    pub update: UpdateStatus,
}

/// A thin status bar rendered at the bottom of the workspace.
#[derive(IntoElement)]
pub struct StatusBar {
    model: StatusBarModel,
    active_side_pane: Option<SidePane>,
    registry: gpui_kit::Entity<ConnectionRegistry>,
}

impl StatusBar {
    pub fn new(
        model: StatusBarModel,
        active_side_pane: Option<SidePane>,
        registry: gpui_kit::Entity<ConnectionRegistry>,
    ) -> Self {
        Self {
            model,
            active_side_pane,
            registry,
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
        SidePane::Chat => "status-chat",
    };
    let (action, tooltip_text) = match pane {
        SidePane::Inspector => (
            &ToggleInspectorPane as &dyn gpui_kit::Action,
            pane.tooltip(),
        ),
        SidePane::History => (&ToggleHistoryPane as &dyn gpui_kit::Action, pane.tooltip()),
        SidePane::Saved => (&ToggleSavedPane as &dyn gpui_kit::Action, pane.tooltip()),
        SidePane::Chat => (&ToggleChatPane as &dyn gpui_kit::Action, pane.tooltip()),
    };

    Button::new(id)
        .ghost()
        .small()
        .icon(Icon::new(pane.icon()).text_color(color))
        .tooltip_with_action(tooltip_text, action, None)
        .on_click(move |_, _, cx| {
            inbox::toggle_side_pane(pane, cx);
            if let Some(ws) = Workspaces::active(cx) {
                ws.update(cx, |_, cx| cx.notify());
            }
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

/// Whether inline completions are on, with what they send; click toggles them.
fn ghost_item(cx: &App) -> impl IntoElement {
    let ai = prefs::ai_prefs(cx);
    let on = ai.inline();
    let color = if on {
        cx.theme().accent_foreground
    } else {
        cx.theme().muted_foreground
    };
    let tooltip = if on {
        format!(
            "Tab completions are on: the text around the cursor goes to {} on each pause. Click to turn off.",
            ai.provider().label()
        )
    } else {
        "Tab completions are off. Click to suggest completions as you type; the text around the cursor goes to the AI provider on each pause.".to_owned()
    };
    Button::new("status-ghost")
        .ghost()
        .small()
        .tooltip(tooltip)
        .on_click(move |_, _, cx| prefs::set_ai_inline(!on, cx))
        .child(
            h_flex()
                .items_center()
                .gap(px(4.0))
                .child(
                    div()
                        .w(px(5.0))
                        .h(px(5.0))
                        .rounded_full()
                        .flex_shrink_0()
                        .bg(color.opacity(if on { 1.0 } else { 0.5 })),
                )
                .child(div().text_xs().text_color(color).child(if on {
                    "Tab completions"
                } else {
                    "Tab completions off"
                })),
        )
}

/// Version label doubling as the update entry point; the click menu depends on the phase.
fn version_item(
    status: &UpdateStatus,
    registry: gpui_kit::Entity<ConnectionRegistry>,
    cx: &App,
) -> impl IntoElement {
    let status = status.clone();
    let locked = update_check_settings_locked();
    let muted = cx.theme().muted_foreground;
    let accent = cx.theme().accent_foreground;
    let warning = cx.theme().warning_foreground;
    let danger = cx.theme().danger_foreground;
    let current = env!("CARGO_PKG_VERSION");
    let new_version: SharedString = status
        .version()
        .map(|v| v.trim_start_matches('v').to_string())
        .unwrap_or_default()
        .into();

    let (suffix, color, tooltip): (Option<String>, _, String) = if locked {
        (None, muted, format!("Based {current}"))
    } else {
        match &status {
            UpdateStatus::Idle => (None, muted, format!("Based {current} — click for options")),
            UpdateStatus::Checking => (
                Some("checking…".into()),
                muted,
                "Checking for updates…".into(),
            ),
            UpdateStatus::UpToDate => (None, muted, format!("Based {current} is up to date")),
            UpdateStatus::Available { .. } => (
                Some(format!("→ {new_version}")),
                accent,
                format!("Based {new_version} is available — click to review"),
            ),
            UpdateStatus::Downloading { percent, .. } => (
                Some(format!("{percent}%")),
                accent,
                format!("Downloading {new_version}… {percent}%"),
            ),
            UpdateStatus::Ready { .. } => (
                Some("restart".into()),
                warning,
                format!("Based {new_version} is downloaded — click to install"),
            ),
            UpdateStatus::Installing { .. } => (
                Some("installing…".into()),
                warning,
                format!("Installing Based {new_version}…"),
            ),
            UpdateStatus::Failed { error } => {
                (Some("update failed".into()), danger, error.to_string())
            }
        }
    };
    let show_dot = matches!(
        status,
        UpdateStatus::Available { .. }
            | UpdateStatus::Downloading { .. }
            | UpdateStatus::Ready { .. }
            | UpdateStatus::Installing { .. }
            | UpdateStatus::Failed { .. }
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
                match &status {
                    UpdateStatus::Installing { .. } => return menu,
                    UpdateStatus::Available { can_install, .. } => {
                        menu = menu.item(notes_new());
                        menu = if *can_install {
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
                    UpdateStatus::Downloading { .. } => return menu.item(notes_new()),
                    UpdateStatus::Ready { .. } => {
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
                                .disabled(status == UpdateStatus::Checking)
                                .on_click(|_, _, cx| updater::check_now(cx)),
                        );
                        if matches!(status, UpdateStatus::Failed { .. }) {
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

        h_flex()
            .h(px(STATUS_BAR_HEIGHT))
            .w_full()
            .px(px(8.0))
            .border_t_1()
            .border_color(cx.theme().border)
            .bg(cx.theme().tab_bar)
            .items_center()
            .justify_between()
            .child(div())
            .child(
                h_flex()
                    .gap(px(6.0))
                    .items_center()
                    .flex_shrink_0()
                    .child(ai_item(cx))
                    .child(ghost_item(cx))
                    .child(status_divider(muted))
                    .child(version_item(&update_snapshot, registry, cx))
                    .child(status_divider(muted))
                    .child(h_flex().gap(px(2.0)).items_center().children(
                        SidePane::ALL.map(|pane| side_pane_button(pane, active_side_pane, cx)),
                    )),
            )
    }
}
