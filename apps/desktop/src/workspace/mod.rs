// workspace/ — Workspace entity, DockArea, sidebar, status bar, connection wiring.

pub mod tab_spec;
pub use tab_spec::TabSpec;

pub mod tab_manager;
pub use tab_manager::TabManager;

pub mod connection_tree;
pub use connection_tree::ConnectionTree;

pub mod item;
pub mod notify;
pub mod object_info;
pub mod pane;
pub mod pop_out;
pub use pop_out::PopOutManager;
pub mod sidebar;
pub mod status_bar;
pub mod topbar;
pub mod welcome;

use crate::widgets::ui::{engine_chip, engine_name, metadata_pill};
use gpui::{
    Context, Entity, FocusHandle, Focusable, FontWeight, IntoElement, Render, SharedString, Window,
    div, prelude::*,
};
use gpui_component::{
    ActiveTheme, StyledExt,
    dock::{DockArea, DockItem, PanelStyle},
    h_flex, v_flex,
};

use crate::bindings::{CycleAppearance, ToggleSidebarRail};
use crate::connection::registry::ConnectionRegistry;
use crate::connection::{ConnectionEntry, ConnectionState, EngineKind};
use crate::project::{find_project_root, load_workspace_seed};

use object_info::ObjectInfoPanel;
use status_bar::StatusBar;
use topbar::Topbar;
use welcome::WelcomePanel;

pub struct Workspace {
    registry: Entity<ConnectionRegistry>,
    dock_area: Entity<DockArea>,
    connection_tree: Entity<ConnectionTree>,
    #[allow(dead_code)]
    tab_manager: Entity<TabManager>,
    sidebar_collapsed: bool,
    inspector_collapsed: bool,
    focus_handle: FocusHandle,
    project_title: SharedString,
}

impl Workspace {
    pub fn new(window: &mut Window, cx: &mut Context<Self>) -> Self {
        let (project_title, entries) = find_project_root()
            .map(|root| {
                let (title, e) = load_workspace_seed(&root);
                (title.into(), e)
            })
            .unwrap_or_else(|| ("No Project".into(), vec![]));

        if entries.is_empty() {
            log::warn!(
                "no connections loaded; add [connection.id] tables to .based/config.toml (or set BASED_PROJECT_DIR)"
            );
        }

        let registry = cx.new(ConnectionRegistry::new);
        registry.update(cx, |reg, cx| {
            for entry in entries {
                reg.add(entry, cx);
            }
        });

        let dock_area = cx.new(|cx| {
            DockArea::new("workspace", Some(1), window, cx).panel_style(PanelStyle::TabBar)
        });

        let welcome = cx.new(|cx| WelcomePanel::new(window, cx));
        let center = DockItem::tab(welcome, &dock_area.downgrade(), window, cx);
        dock_area.update(cx, |area, cx| {
            area.set_center(center, window, cx);
        });

        let connection_tree =
            cx.new(|cx| ConnectionTree::new(registry.clone(), dock_area.clone(), cx));

        let tab_manager = cx.new(|_| TabManager::new());

        let tree_observe = connection_tree.clone();

        let workspace = Self {
            registry: registry.clone(),
            dock_area,
            connection_tree,
            tab_manager,
            sidebar_collapsed: crate::app::prefs::collapsed_from(cx),
            inspector_collapsed: false,
            focus_handle: cx.focus_handle(),
            project_title,
        };

        // Detach so subscriptions survive past `new` — dropping `Subscription` unsubscribes.
        cx.observe(&registry, |_ws, _reg, cx| {
            cx.notify();
        })
        .detach();
        cx.observe(&tree_observe, |_ws, _, cx| {
            cx.notify();
        })
        .detach();

        workspace
    }

    pub fn toggle_sidebar_rail(&mut self, cx: &mut Context<Self>) {
        self.sidebar_collapsed = !self.sidebar_collapsed;
        crate::app::prefs::set_sidebar(self.sidebar_collapsed, cx);
        cx.notify();
    }
}

impl Focusable for Workspace {
    fn focus_handle(&self, _: &gpui::App) -> FocusHandle {
        self.focus_handle.clone()
    }
}

impl Render for Workspace {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let this = cx.entity().clone();
        let conn_list: Vec<Entity<ConnectionEntry>> = self.registry.read(cx).connections().to_vec();
        let conn_count = conn_list.len();
        let connected_count = conn_list
            .iter()
            .filter(|ent| matches!(ent.read(cx).state, ConnectionState::Connected(_)))
            .count();
        let border = cx.theme().sidebar_border;
        let sidebar_bg = cx.theme().sidebar;

        let sidebar = v_flex()
            .w(gpui::px(274.0))
            .h_full()
            .flex_shrink_0()
            .border_r_1()
            .border_color(border)
            .bg(sidebar_bg)
            .child(self.connection_tree.clone());

        let dock_host = div()
            .flex_1()
            .size_full()
            .overflow_hidden()
            .child(self.dock_area.clone());

        let selected_connection = self.connection_tree.read(cx).selected_connection_entry(cx);
        let inspector = render_inspector(selected_connection, cx);

        let body = if self.sidebar_collapsed {
            h_flex()
                .flex_1()
                .overflow_hidden()
                .child(dock_host)
                .when(!self.inspector_collapsed, |row| row.child(inspector))
        } else {
            h_flex()
                .flex_1()
                .overflow_hidden()
                .child(sidebar)
                .child(dock_host)
                .when(!self.inspector_collapsed, |row| row.child(inspector))
        };

        v_flex()
            .size_full()
            .track_focus(&self.focus_handle)
            .on_action(
                window.listener_for(&this, |ws, _: &ToggleSidebarRail, _, cx| {
                    ws.toggle_sidebar_rail(cx);
                }),
            )
            .on_action(window.listener_for(&this, |_, _: &CycleAppearance, _, cx| {
                crate::app::prefs::cycle_theme(cx);
            }))
            .bg(cx.theme().background)
            .child(Topbar::new(
                self.project_title.clone(),
                this.clone(),
                conn_count,
                connected_count,
            ))
            .child(body)
            .child(StatusBar::new(conn_count, connected_count))
    }
}

fn render_inspector(
    selected: Option<Entity<ConnectionEntry>>,
    cx: &mut Context<Workspace>,
) -> impl IntoElement {
    let border = cx.theme().border;
    let muted = cx.theme().muted_foreground;

    let content = if let Some(ent) = selected {
        let entry = ent.read(cx);
        let engine = entry.config.engine();
        let label = entry.config.label().to_string();
        let state = entry.state.label().to_string();
        let summary = match &entry.state {
            ConnectionState::Failed { reason, .. } => notify::error_one_liner(reason).to_string(),
            ConnectionState::Connected(_) => "Ready for browsing and queries".to_string(),
            ConnectionState::Connecting { .. } => "Opening connection".to_string(),
            ConnectionState::Disconnected => "Click to connect".to_string(),
        };

        v_flex()
            .gap_3()
            .p_3()
            .child(
                v_flex()
                    .gap_1()
                    .child(
                        div()
                            .text_sm()
                            .font_weight(FontWeight::SEMIBOLD)
                            .text_color(cx.theme().foreground)
                            .truncate()
                            .child(label),
                    )
                    .child(
                        div()
                            .text_xs()
                            .text_color(muted)
                            .child(format!("{} connection", engine_name(engine))),
                    ),
            )
            .child(
                h_flex()
                    .gap_2()
                    .child(engine_chip(engine, cx))
                    .child(metadata_pill("state", state, cx)),
            )
            .child(inspector_section(
                "Activity",
                vec![
                    ("Recent", "Schema refresh"),
                    ("Saved", "0 queries"),
                    ("Pinned", "No pinned objects"),
                ],
                cx,
            ))
            .child(inspector_note("Health", &summary, cx))
            .into_any_element()
    } else {
        v_flex()
            .gap_3()
            .p_3()
            .child(inspector_note(
                "Selection",
                "Choose a connection, table, cell, or query to see details here.",
                cx,
            ))
            .child(inspector_section(
                "Shortcuts",
                vec![
                    (
                        "Command",
                        if cfg!(target_os = "macos") {
                            "⌘K"
                        } else {
                            "Ctrl K"
                        },
                    ),
                    (
                        "Run query",
                        if cfg!(target_os = "macos") {
                            "⌘↵"
                        } else {
                            "Ctrl Enter"
                        },
                    ),
                    (
                        "Sidebar",
                        if cfg!(target_os = "macos") {
                            "⌘\\"
                        } else {
                            "Ctrl \\"
                        },
                    ),
                ],
                cx,
            ))
            .into_any_element()
    };

    v_flex()
        .w(gpui::px(286.0))
        .h_full()
        .flex_shrink_0()
        .border_l_1()
        .border_color(border)
        .bg(cx.theme().background)
        .child(
            h_flex()
                .h(gpui::px(38.0))
                .px_3()
                .items_center()
                .border_b_1()
                .border_color(border.opacity(0.86))
                .child(
                    div()
                        .text_xs()
                        .font_bold()
                        .font_family(cx.theme().mono_font_family.clone())
                        .text_color(muted)
                        .child("INSPECTOR"),
                ),
        )
        .child(content)
}

fn inspector_note(
    title: &'static str,
    body: &str,
    cx: &mut Context<Workspace>,
) -> impl IntoElement {
    v_flex()
        .gap_1()
        .p_2()
        .rounded(gpui::px(7.0))
        .border_1()
        .border_color(cx.theme().border.opacity(0.82))
        .bg(cx.theme().muted.opacity(0.28))
        .child(
            div()
                .text_xs()
                .font_weight(FontWeight::SEMIBOLD)
                .text_color(cx.theme().foreground)
                .child(title),
        )
        .child(
            div()
                .text_xs()
                .text_color(cx.theme().muted_foreground)
                .child(body.to_string()),
        )
}

fn inspector_section(
    title: &'static str,
    rows: Vec<(&'static str, &'static str)>,
    cx: &mut Context<Workspace>,
) -> impl IntoElement {
    v_flex()
        .gap_2()
        .child(
            div()
                .text_xs()
                .font_bold()
                .font_family(cx.theme().mono_font_family.clone())
                .text_color(cx.theme().muted_foreground)
                .child(title),
        )
        .children(rows.into_iter().map(|(label, value)| {
            h_flex()
                .h(gpui::px(24.0))
                .items_center()
                .justify_between()
                .border_b_1()
                .border_color(cx.theme().border.opacity(0.42))
                .child(
                    div()
                        .text_xs()
                        .text_color(cx.theme().muted_foreground)
                        .child(label),
                )
                .child(
                    div()
                        .text_xs()
                        .text_color(cx.theme().foreground.opacity(0.88))
                        .truncate()
                        .child(value),
                )
        }))
}
