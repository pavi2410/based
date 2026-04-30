use gpui::{
    App, Entity, IntoElement, ParentElement, RenderOnce, SharedString, Styled, div,
};
use gpui_component::{
    ActiveTheme as _, IconName, Sizable as _, StyledExt, TitleBar,
    button::{Button, ButtonVariants},
    h_flex,
};

use super::Workspace;
use crate::app::prefs;

/// A `RenderOnce` top bar that renders inside the window's `TitleBar`.
#[derive(IntoElement)]
pub struct Topbar {
    pub project_name: SharedString,
    pub workspace: Entity<Workspace>,
}

impl Topbar {
    pub fn new(project_name: impl Into<SharedString>, workspace: Entity<Workspace>) -> Self {
        Self {
            project_name: project_name.into(),
            workspace,
        }
    }
}

impl RenderOnce for Topbar {
    fn render(self, _: &mut gpui::Window, cx: &mut App) -> impl IntoElement {
        let workspace = self.workspace.clone();
        let workspace_rail = workspace.clone();

        let collapsed = prefs::collapsed_from(cx);
        let is_dark = cx.theme().is_dark();

        TitleBar::new().child(
            h_flex()
                .w_full()
                .items_center()
                .justify_between()
                .gap_1()
                .child(
                    h_flex()
                        .gap_0()
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
                    h_flex().flex_1().items_center().justify_center().child(
                        div()
                            .text_sm()
                            .font_semibold()
                            .font_family(cx.theme().mono_font_family.clone())
                            .text_color(cx.theme().foreground)
                            .child(self.project_name.clone()),
                    ),
                )
                .child(
                    Button::new("settings")
                        .ghost()
                        .xsmall()
                        .icon(IconName::Settings)
                        .tooltip(SharedString::from("Settings — coming soon"))
                        .on_click(|_, _, _| eprintln!("settings — Phase 2 will wire this")),
                ),
        )
    }
}
