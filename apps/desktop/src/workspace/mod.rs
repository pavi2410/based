// workspace/ — Workspace entity, DockArea, sidebar, status bar, connection wiring.

pub mod session;
pub mod tab_label;
pub mod tab_open;
pub mod tab_spec;
pub use tab_label::tab_label_for_spec;
pub use tab_open::{
    SqlInject, TabManagerRef, TabOpenQueue, WorkspaceRef, enqueue_open_tab, enqueue_sql_inject,
    mark_query_tab_dirty,
};
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

mod dock_utils;
mod inspector;
mod overlays;
mod pop_out_impls;
mod tab_dispatch;

use dock_utils::{active_center_tab, center_tab_items, dock_area_present_views};
use inspector::render_inspector;

use std::path::PathBuf;

use gpui::{
    App, Bounds, Context, Entity, EntityId, FocusHandle, Focusable, IntoElement, Render,
    SharedString, Window, WindowBounds, WindowOptions, div, point, prelude::*, px, size,
};
use gpui_component::{
    ActiveTheme, Root,
    dock::{DockArea, DockEvent, DockItem, DockPlacement, PanelStyle},
    h_flex, v_flex,
};

use crate::bindings::{
    CloseTab, CycleAppearance, DismissCommandPalette, OpenSettings, ToggleCommandPalette,
    ToggleSidebarRail,
};
use crate::command_palette::CommandPalette;
use crate::connection::registry::ConnectionRegistry;
use crate::connection::{ConnectionEntry, ConnectionId, ConnectionState};
use crate::project::{find_project_root, load_workspace_seed};

use status_bar::StatusBar;
use topbar::Topbar;
use welcome::WelcomePanel;

pub struct Workspace {
    registry: Entity<ConnectionRegistry>,
    dock_area: Entity<DockArea>,
    connection_tree: Entity<ConnectionTree>,
    tab_manager: Entity<TabManager>,
    command_palette: Entity<CommandPalette>,
    sidebar_collapsed: bool,
    inspector_collapsed: bool,
    focus_handle: FocusHandle,
    project_title: SharedString,
    project_dir: Option<PathBuf>,
    session_restored: bool,
    pending_open_tab: Option<TabSpec>,
    /// Set by platform close; dialog is shown on the next [`Render`] (see `app::quit`).
    pub(crate) pending_close_confirm: bool,
}

impl Workspace {
    pub fn new(window: &mut Window, cx: &mut Context<Self>) -> Self {
        let project_dir = find_project_root();
        let (project_title, entries) = project_dir
            .as_ref()
            .map(|root| {
                let (title, e) = load_workspace_seed(root);
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
        cx.set_global(TabManagerRef(tab_manager.clone()));
        if let Some(root) = project_dir.clone() {
            cx.set_global(crate::project::RegistryRef(registry.clone()));
            cx.set_global(crate::project::ProjectRoot(root));
        }
        let command_palette =
            cx.new(|cx| CommandPalette::new(registry.clone(), connection_tree.clone(), cx));
        let palette_observe = command_palette.clone();

        let tree_observe = connection_tree.clone();

        let workspace = Self {
            registry: registry.clone(),
            dock_area,
            connection_tree,
            tab_manager,
            command_palette,
            sidebar_collapsed: crate::app::prefs::collapsed_from(cx),
            inspector_collapsed: false,
            focus_handle: cx.focus_handle(),
            project_title,
            project_dir,
            session_restored: false,
            pending_open_tab: None,
            pending_close_confirm: false,
        };

        cx.subscribe(&tree_observe, |ws, _, event, ecx| {
            let connection_tree::TreeEvent::OpenTab(spec) = event;
            ws.pending_open_tab = Some(spec.clone());
            ecx.notify();
        })
        .detach();

        cx.subscribe(&palette_observe, |ws, _, event, ecx| {
            match event {
                crate::command_palette::PaletteEvent::OpenTab(spec) => {
                    ws.pending_open_tab = Some(spec.clone());
                }
                crate::command_palette::PaletteEvent::InjectSql { conn_id, sql } => {
                    let active_matches = ws.tab_manager.read(ecx).active_tab().is_some_and(|t| {
                        matches!(
                            &t.spec,
                            TabSpec::QueryEditor {
                                conn_id: active, ..
                            } if active == conn_id
                        )
                    });
                    if active_matches {
                        enqueue_sql_inject(conn_id.clone(), sql.clone(), ecx);
                    } else {
                        ws.pending_open_tab = Some(TabSpec::QueryEditor {
                            conn_id: conn_id.clone(),
                            initial_sql: Some(sql.clone()),
                            initial_pipeline: None,
                            mongo_collection: None,
                            auto_run: false,
                        });
                    }
                }
            }
            ecx.notify();
        })
        .detach();

        // Detach so subscriptions survive past `new` — dropping `Subscription` unsubscribes.
        cx.observe(&registry, |_ws, _reg, cx| {
            cx.notify();
        })
        .detach();
        cx.observe(&tree_observe, |_ws, _, cx| {
            cx.notify();
        })
        .detach();

        let dock_observe = workspace.dock_area.clone();
        cx.subscribe(&dock_observe, |ws, _, event: &DockEvent, ecx| {
            if matches!(event, DockEvent::LayoutChanged) {
                ws.sync_tab_manager_from_dock(ecx);
            }
        })
        .detach();

        let tab_mgr_observe = workspace.tab_manager.clone();
        cx.subscribe(&tab_mgr_observe, |ws, _, _: &tab_manager::TabEvent, ecx| {
            ws.save_session(ecx);
        })
        .detach();

        let registry_for_close = registry.clone();
        let workspace_for_close = cx.entity();
        window.on_window_should_close(cx, move |window, cx| {
            let result = crate::app::quit::confirm_before_close_window(
                &registry_for_close,
                &workspace_for_close,
                window,
                cx,
            );
            log::warn!(
                target: "based_quit",
                "on_window_should_close handler returning allow_close={result}"
            );
            result
        });
        log::warn!(target: "based_quit", "registered on_window_should_close for main workspace");

        workspace
    }

    pub fn registry(&self) -> &Entity<ConnectionRegistry> {
        &self.registry
    }

    fn save_session(&self, cx: &Context<Self>) {
        let Some(root) = self.project_dir.as_ref() else {
            return;
        };
        let tm = self.tab_manager.read(cx);
        if tm.tabs.is_empty() {
            return;
        }
        let state = session::SessionState {
            tabs: tm.tabs.iter().map(|t| t.spec.clone()).collect(),
            active: tm.active_idx,
        };
        let _ = state.save(root);
    }

    fn restore_session(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let Some(root) = self.project_dir.clone() else {
            return;
        };
        let session = session::SessionState::load(&root);
        for spec in session.tabs {
            self.pending_open_tab = Some(spec);
            self.flush_pending_open_tab(window, cx);
        }
        if let Some(idx) = session.active {
            self.tab_manager.update(cx, |tm, ecx| {
                if idx < tm.tabs.len() {
                    tm.activate(idx, ecx);
                }
            });
        }
    }

    pub fn toggle_sidebar_rail(&mut self, cx: &mut Context<Self>) {
        self.sidebar_collapsed = !self.sidebar_collapsed;
        crate::app::prefs::set_sidebar(self.sidebar_collapsed, cx);
        cx.notify();
    }

    pub fn open_settings(&mut self, _window: &mut Window, cx: &mut Context<Self>) {
        let _ = cx.open_window(
            WindowOptions {
                window_bounds: Some(WindowBounds::Windowed(Bounds {
                    origin: point(px(120.0), px(120.0)),
                    size: size(px(800.0), px(600.0)),
                })),
                titlebar: Some(crate::app::shell::titled_titlebar("Based — Settings")),
                ..Default::default()
            },
            |win, cx| {
                win.set_window_title("Based — Settings");
                let settings = cx.new(crate::settings_window::SettingsWindow::new);
                cx.new(|cx| Root::new(settings, win, cx))
            },
        );
    }

    fn drain_tab_open_queue(&mut self, cx: &mut Context<Self>) {
        if let Some(spec) = cx.update_global(|q: &mut TabOpenQueue, _| q.pending.take()) {
            self.pending_open_tab = Some(spec);
        }
    }

    fn flush_pending_open_tab(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.drain_tab_open_queue(cx);
        if let Some(spec) = self.pending_open_tab.take() {
            self.dispatch_open_tab(spec, window, cx);
        }
    }

    fn sync_tab_manager_from_dock(&mut self, cx: &mut Context<Self>) {
        let views = {
            let dock = self.dock_area.read(cx);
            dock_area_present_views(dock, cx)
        };
        self.tab_manager.update(cx, |tm, ecx| {
            tm.sync_open_tabs(&views, ecx);
        });
    }

    /// Number of panels in the center tab strip (dock layout only; does not read panel entities).
    pub fn center_tab_count(&self, cx: &App) -> usize {
        let dock = self.dock_area.read(cx);
        center_tab_items(dock.center())
            .map(|items| items.len())
            .unwrap_or(0)
    }

    /// Whether the given center-dock panel may be closed (gpui-component hides Close for center tabs).
    pub fn can_close_center_panel(&self, panel_id: EntityId, cx: &App) -> bool {
        if self.center_tab_count(cx) <= 1 {
            return false;
        }
        let dock = self.dock_area.read(cx);
        let Some(items) = center_tab_items(dock.center()) else {
            return false;
        };
        let Some(panel) = items.iter().find(|p| p.panel_id(cx) == panel_id) else {
            return false;
        };
        panel.closable(cx)
    }

    /// Close a specific panel in the center tab strip (tab ⋮ menu).
    pub fn close_center_panel(
        &mut self,
        panel_id: EntityId,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if !self.can_close_center_panel(panel_id, cx) {
            return;
        }
        let dock = self.dock_area.read(cx);
        let Some(items) = center_tab_items(dock.center()) else {
            return;
        };
        let Some(panel) = items.iter().find(|p| p.panel_id(cx) == panel_id).cloned() else {
            return;
        };
        self.dock_area.update(cx, |dock, ecx| {
            dock.remove_panel(panel, DockPlacement::Center, window, ecx);
        });
    }

    /// Close the active center tab (⌘W / CloseTab).
    pub fn close_active_center_tab(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let dock = self.dock_area.read(cx);
        let Some((panel, _)) = active_center_tab(dock.center(), cx) else {
            return;
        };
        self.close_center_panel(panel.panel_id(cx), window, cx);
    }
}

impl Focusable for Workspace {
    fn focus_handle(&self, _: &gpui::App) -> FocusHandle {
        self.focus_handle.clone()
    }
}

impl Render for Workspace {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        crate::app::quit::maybe_show_pending_close_dialog(self, window, cx);
        crate::project::drain_pending_reload(cx);
        if !self.session_restored {
            self.session_restored = true;
            self.restore_session(window, cx);
        }
        self.flush_pending_open_tab(window, cx);
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
            .min_h_0()
            .flex_shrink_0()
            .overflow_hidden()
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

        let main = v_flex()
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
            .on_action(
                window.listener_for(&this, |ws, _: &ToggleCommandPalette, window, cx| {
                    ws.command_palette.update(cx, |p, cx| p.toggle(window, cx));
                }),
            )
            .on_action(
                window.listener_for(&this, |ws, _: &DismissCommandPalette, _, cx| {
                    ws.command_palette.update(cx, |p, cx| {
                        if p.is_visible() {
                            p.dismiss(cx);
                        }
                    });
                }),
            )
            .on_action(
                window.listener_for(&this, |ws, _: &OpenSettings, window, cx| {
                    ws.open_settings(window, cx);
                }),
            )
            .on_action(window.listener_for(&this, |ws, _: &CloseTab, window, cx| {
                ws.close_active_center_tab(window, cx);
            }))
            .bg(cx.theme().background)
            .child(Topbar::new(
                self.project_title.clone(),
                this.clone(),
                conn_count,
                connected_count,
            ))
            .child(body)
            .child(StatusBar::new(status_bar::StatusBarModel {
                connection_count: conn_count,
                connected_count,
                scope_label: self.project_title.clone(),
                history_ready: !cx
                    .global::<crate::query_store::QueryStore>()
                    .history
                    .recent(1)
                    .is_empty(),
            }))
            .child(self.command_palette.clone());

        overlays::stack_gpui_overlays(main, window, cx)
    }
}
