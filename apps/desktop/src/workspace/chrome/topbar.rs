use gpui::{App, Entity, IntoElement, ParentElement, RenderOnce, SharedString, Styled, div, px};
use gpui_component::{
    ActiveTheme as _, IconName, Sizable as _, TitleBar,
    button::{Button, ButtonVariants},
    h_flex,
    select::{Select, SelectState},
};

use crate::app::prefs;
use crate::bindings::{CycleAppearance, ToggleSidebarRail};
use crate::widgets::ui::command_shell;
use crate::workspace::Workspace;

/// A `RenderOnce` top bar that renders inside the window's `TitleBar`.
#[derive(IntoElement)]
pub struct Topbar {
    pub project_name: SharedString,
    pub workspace: Entity<Workspace>,
    pub env_select: Entity<SelectState<Vec<&'static str>>>,
}

impl Topbar {
    pub fn new(
        project_name: impl Into<SharedString>,
        workspace: Entity<Workspace>,
        env_select: Entity<SelectState<Vec<&'static str>>>,
    ) -> Self {
        Self {
            project_name: project_name.into(),
            workspace,
            env_select,
        }
    }
}

impl RenderOnce for Topbar {
    fn render(self, _window: &mut gpui::Window, cx: &mut App) -> impl IntoElement {
        let registry = self.workspace.read(cx).registry().clone();

        TitleBar::new()
            .on_close_window({
                let registry = registry.clone();
                move |_, window, cx| {
                    crate::app::quit::request_window_close(&registry, window, cx);
                }
            })
            .child(
                h_flex()
                    .w_full()
                    .items_center()
                    .gap_2()
                    .child(TopbarLeft {
                        project_name: self.project_name,
                        workspace: self.workspace.clone(),
                        env_select: self.env_select,
                    })
                    .child(TopbarCenter)
                    .child(TopbarRight {
                        workspace: self.workspace,
                    }),
            )
    }
}

/// Title bar left rail: sidebar toggle, project name, env selector.
#[derive(IntoElement)]
struct TopbarLeft {
    project_name: SharedString,
    workspace: Entity<Workspace>,
    env_select: Entity<SelectState<Vec<&'static str>>>,
}

impl RenderOnce for TopbarLeft {
    fn render(self, _: &mut gpui::Window, cx: &mut App) -> impl IntoElement {
        let workspace_rail = self.workspace.clone();
        let collapsed = prefs::collapsed_from(cx);

        h_flex()
            .flex_1()
            .items_center()
            .justify_start()
            .gap_2()
            .child(
                Button::new("rail-toggle")
                    .ghost()
                    .small()
                    .icon(if collapsed {
                        IconName::PanelLeftOpen
                    } else {
                        IconName::PanelLeftClose
                    })
                    .tooltip_with_action("Toggle connections", &ToggleSidebarRail, None)
                    .on_click(move |_, _, cx| {
                        let ent = workspace_rail.clone();
                        ent.update(cx, |ws, cx| {
                            ws.toggle_sidebar_rail(cx);
                        });
                    }),
            )
            .child(
                div()
                    .flex_shrink_0()
                    .max_w(px(200.0))
                    .text_sm()
                    .font_family(cx.theme().mono_font_family.clone())
                    .text_color(cx.theme().foreground)
                    .truncate()
                    .child(self.project_name),
            )
            .child(
                Select::new(&self.env_select)
                    .small()
                    .title_prefix("env ")
                    .w(px(104.0)),
            )
    }
}

/// Title bar center: command palette trigger.
#[derive(IntoElement)]
struct TopbarCenter;

impl RenderOnce for TopbarCenter {
    fn render(self, window: &mut gpui::Window, cx: &mut App) -> impl IntoElement {
        command_shell(window, cx, "Search tables, queries, history…")
    }
}

/// Title bar right rail: theme toggle and settings.
#[derive(IntoElement)]
struct TopbarRight {
    workspace: Entity<Workspace>,
}

impl RenderOnce for TopbarRight {
    fn render(self, _window: &mut gpui::Window, cx: &mut App) -> impl IntoElement {
        let workspace_settings = self.workspace.clone();
        let is_dark = cx.theme().is_dark();

        h_flex()
            .flex_1()
            .min_w_0()
            .items_center()
            .justify_end()
            .gap_1()
            .child(
                Button::new("appearance")
                    .ghost()
                    .small()
                    .icon(if is_dark {
                        IconName::Sun
                    } else {
                        IconName::Moon
                    })
                    .tooltip_with_action("Appearance", &CycleAppearance, None)
                    .on_click(move |_, _, cx| {
                        prefs::cycle_theme(cx);
                    }),
            )
            .child(
                Button::new("settings")
                    .ghost()
                    .small()
                    .icon(IconName::Settings)
                    .tooltip(SharedString::from("Settings"))
                    .on_click(move |_, window, cx| {
                        let ent = workspace_settings.clone();
                        ent.update(cx, |ws, cx| {
                            ws.open_settings(window, cx);
                        });
                    }),
            )
    }
}
