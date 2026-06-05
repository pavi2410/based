use gpui::{App, Entity, IntoElement, ParentElement, RenderOnce, SharedString, Styled, div, px};
use gpui_component::{
    ActiveTheme as _, Icon, IconName, Sizable as _, TitleBar,
    button::{Button, ButtonVariants},
    h_flex,
    menu::{DropdownMenu, PopupMenuItem},
};

const GIT_BRANCH_ICON_PATH: &str = "icons/git-branch.svg";

use crate::app::{prefs, shell};
use crate::bindings::CycleAppearance;
use crate::connection::registry::ConnectionRegistry;
use crate::project::ProjectContext;
use crate::widgets::{SIDEBAR_INSET, command_shell};

/// A `RenderOnce` top bar that renders inside the window's `TitleBar`.
#[derive(IntoElement)]
pub struct Topbar {
    pub registry: Entity<ConnectionRegistry>,
}

impl Topbar {
    pub fn new(registry: Entity<ConnectionRegistry>) -> Self {
        Self { registry }
    }
}

impl RenderOnce for Topbar {
    fn render(self, _window: &mut gpui::Window, _cx: &mut App) -> impl IntoElement {
        let registry = self.registry;

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
                    .child(ContextRail)
                    .child(TopbarCenter)
                    .child(TopbarRight),
            )
    }
}

/// Zed-style `[ Project | Branch | Env ]` breadcrumb rail.
#[derive(IntoElement)]
struct ContextRail;

impl RenderOnce for ContextRail {
    fn render(self, _: &mut gpui::Window, cx: &mut App) -> impl IntoElement {
        let muted = cx.theme().muted_foreground;
        let (project_name, project_path, branch, env) = cx
            .try_global::<ProjectContext>()
            .map(|ctx| {
                (
                    ctx.project_name().to_string(),
                    ctx.root.display().to_string(),
                    ctx.git_branch.clone().unwrap_or_else(|| "—".into()),
                    ctx.active_env().to_string(),
                )
            })
            .unwrap_or_else(|| {
                (
                    "No project".into(),
                    String::new(),
                    "—".into(),
                    "default".into(),
                )
            });

        h_flex()
            .flex_1()
            .items_center()
            .gap_1()
            .child(
                Button::new("ctx-project")
                    .ghost()
                    .small()
                    .child(
                        h_flex().items_center().gap_1().child(
                            div()
                                .text_xs()
                                .font_weight(gpui::FontWeight::SEMIBOLD)
                                .text_color(cx.theme().foreground)
                                .child(SharedString::from(project_name)),
                        ),
                    )
                    .tooltip(SharedString::from(if project_path.is_empty() {
                        "Open a folder containing .based/".into()
                    } else {
                        project_path
                    }))
                    .dropdown_menu(move |menu, _window, _cx| {
                        menu.item(
                            PopupMenuItem::new("Open Folder…")
                                .icon(IconName::FolderOpen)
                                .on_click(|_, _, cx| {
                                    crate::project::prompt_open_project_in_window(cx)
                                }),
                        )
                        .item(
                            PopupMenuItem::new("Open Folder in New Window…")
                                .icon(IconName::FolderOpen)
                                .on_click(|_, _, cx| {
                                    crate::project::prompt_open_project_in_new_window(cx);
                                }),
                        )
                    }),
            )
            .child(div().text_xs().text_color(muted).child("/"))
            .child(
                Button::new("ctx-branch")
                    .ghost()
                    .small()
                    .icon(Icon::empty().path(GIT_BRANCH_ICON_PATH))
                    .label(branch.clone())
                    .tooltip(SharedString::from("Git branch (read-only)"))
                    .dropdown_menu({
                        let branch_item = branch.clone();
                        move |menu, _window, _cx| {
                            menu.item(
                                PopupMenuItem::new(SharedString::from(branch_item.clone()))
                                    .disabled(true),
                            )
                        }
                    }),
            )
            .child(div().text_xs().text_color(muted).child("/"))
            .child(
                Button::new("ctx-env")
                    .ghost()
                    .small()
                    .icon(IconName::Globe)
                    .label(env.clone())
                    .tooltip(SharedString::from("Active environment"))
                    .dropdown_menu({
                        let env_item = env.clone();
                        move |menu, _window, _cx| {
                            menu.item(
                                PopupMenuItem::new(SharedString::from(env_item.clone()))
                                    .disabled(true),
                            )
                        }
                    }),
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

/// Title bar right rail: theme toggle, app overflow menu (About / Settings).
#[derive(IntoElement)]
struct TopbarRight;

impl RenderOnce for TopbarRight {
    fn render(self, _window: &mut gpui::Window, cx: &mut App) -> impl IntoElement {
        let is_dark = cx.theme().is_dark();

        h_flex()
            .flex_1()
            .min_w_0()
            .items_center()
            .justify_end()
            .gap_1()
            .pr(px(SIDEBAR_INSET))
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
                Button::new("app-overflow")
                    .ghost()
                    .small()
                    .icon(IconName::Ellipsis)
                    .tooltip(SharedString::from("Menu"))
                    .dropdown_menu(|menu, _window, _cx| {
                        let mut menu = menu
                            .item(
                                PopupMenuItem::new("About Based")
                                    .icon(IconName::Info)
                                    .on_click(|_, _window, cx| shell::open_about(cx)),
                            )
                            .item(PopupMenuItem::separator())
                            .item(
                                PopupMenuItem::new("Settings...")
                                    .icon(IconName::Settings)
                                    .on_click(|_, _window, cx| shell::open_settings(cx)),
                            );
                        if crate::app::prefs::manual_update_checks_enabled() {
                            menu = menu.item(
                                PopupMenuItem::new("Check for Updates…")
                                    .icon(IconName::Inbox)
                                    .on_click(|_, _window, cx| crate::app::updater::check_now(cx)),
                            );
                        }
                        menu.item(PopupMenuItem::separator())
                            .item(
                                PopupMenuItem::new("Show Home")
                                    .icon(IconName::BookOpen)
                                    .on_click(|_, _window, cx| shell::open_home(cx)),
                            )
                            .item(
                                PopupMenuItem::new("Onboarding...")
                                    .icon(IconName::Settings2)
                                    .on_click(|_, _window, cx| shell::open_onboarding(cx)),
                            )
                            .item(
                                PopupMenuItem::new("Release Notes")
                                    .icon(IconName::BookOpen)
                                    .on_click(|_, _window, cx| {
                                        crate::app::updater::open_release_notes_for_current(cx);
                                    }),
                            )
                    }),
            )
    }
}
