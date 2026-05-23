use gpui::{App, Entity, IntoElement, ParentElement, RenderOnce, SharedString, Styled, div, px};
use gpui_component::{
    ActiveTheme as _, IconName, Sizable as _, StyledExt, TitleBar,
    button::{Button, ButtonVariants},
    h_flex,
};

use super::Workspace;
use crate::app::prefs;
use crate::widgets::ui::{command_shell, metadata_pill, toolbar_button};

/// A `RenderOnce` top bar that renders inside the window's `TitleBar`.
#[derive(IntoElement)]
pub struct Topbar {
    pub project_name: SharedString,
    pub workspace: Entity<Workspace>,
    pub connection_count: usize,
    pub connected_count: usize,
}

impl Topbar {
    pub fn new(
        project_name: impl Into<SharedString>,
        workspace: Entity<Workspace>,
        connection_count: usize,
        connected_count: usize,
    ) -> Self {
        Self {
            project_name: project_name.into(),
            workspace,
            connection_count,
            connected_count,
        }
    }
}

impl RenderOnce for Topbar {
    fn render(self, _: &mut gpui::Window, cx: &mut App) -> impl IntoElement {
        let workspace = self.workspace.clone();
        let workspace_rail = workspace.clone();
        let workspace_settings = workspace.clone();

        let collapsed = prefs::collapsed_from(cx);
        let is_dark = cx.theme().is_dark();

        let health = if self.connection_count == 0 {
            "No connections".to_string()
        } else {
            format!("{}/{} live", self.connected_count, self.connection_count)
        };

        TitleBar::new().child(
            h_flex()
                .w_full()
                .items_center()
                .justify_between()
                .gap_2()
                .child(
                    h_flex()
                        .gap_1()
                        .items_center()
                        .child(
                            Button::new("rail-toggle")
                                .ghost()
                                .xsmall()
                                .icon(if collapsed {
                                    IconName::PanelLeftOpen
                                } else {
                                    IconName::PanelLeftClose
                                })
                                .tooltip(SharedString::from(if cfg!(target_os = "macos") {
                                    "Toggle connections (⌘\\)"
                                } else {
                                    "Toggle connections (Ctrl+\\)"
                                }))
                                .on_click(move |_, _, cx| {
                                    let ent = workspace_rail.clone();
                                    let _ = ent.update(cx, |ws, cx| {
                                        ws.toggle_sidebar_rail(cx);
                                    });
                                }),
                        )
                        .child(
                            Button::new("appearance")
                                .ghost()
                                .xsmall()
                                .icon(if is_dark {
                                    IconName::Sun
                                } else {
                                    IconName::Moon
                                })
                                .tooltip(SharedString::from(if cfg!(target_os = "macos") {
                                    "Appearance (⇧⌥⌘T)"
                                } else {
                                    "Appearance (Ctrl+Alt+Shift+T)"
                                }))
                                .on_click(move |_, _, cx| {
                                    prefs::cycle_theme(cx);
                                }),
                        ),
                )
                .child(
                    h_flex()
                        .min_w_0()
                        .w(px(220.0))
                        .items_center()
                        .gap_2()
                        .child(
                            div()
                                .text_sm()
                                .font_semibold()
                                .font_family(cx.theme().mono_font_family.clone())
                                .text_color(cx.theme().foreground)
                                .truncate()
                                .child(self.project_name.clone()),
                        )
                        .child(metadata_pill("env", "local", cx)),
                )
                .child(
                    h_flex()
                        .flex_1()
                        .items_center()
                        .justify_center()
                        .child(command_shell(cx, "Search tables, queries, history…")),
                )
                .child(
                    h_flex()
                        .w(px(300.0))
                        .items_center()
                        .justify_end()
                        .gap_1()
                        .child(metadata_pill("workspace", health, cx))
                        .child(toolbar_button(
                            "new-connection",
                            IconName::Plus,
                            "New connection — coming soon",
                        ))
                        .child(toolbar_button(
                            "refresh-workspace",
                            IconName::Search,
                            "Refresh workspace",
                        ))
                        .child(
                            Button::new("settings")
                                .ghost()
                                .xsmall()
                                .icon(IconName::Settings)
                                .tooltip(SharedString::from("Settings"))
                                .on_click(move |_, window, cx| {
                                    let ent = workspace_settings.clone();
                                    let _ = ent.update(cx, |ws, cx| {
                                        ws.open_settings(window, cx);
                                    });
                                }),
                        ),
                ),
        )
    }
}
