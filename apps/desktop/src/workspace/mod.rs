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
mod pop_out_impls;
pub use pop_out::PopOutManager;
pub mod sidebar;
pub mod status_bar;
pub mod topbar;
pub mod welcome;

use std::path::PathBuf;
use std::sync::Arc;

use crate::widgets::ui::{engine_chip, engine_name, metadata_pill};
use gpui::{
    AnyView, App, Bounds, Context, Entity, EntityId, FocusHandle, Focusable, FontWeight,
    IntoElement, Render, SharedString, Window, WindowBounds, WindowOptions, div, point, prelude::*,
    px, size,
};
use gpui_component::{
    ActiveTheme, Root, StyledExt, TitleBar,
    dock::{DockArea, DockEvent, DockItem, DockPlacement, PanelStyle, PanelView},
    h_flex, v_flex,
};

use ::mongodb::bson::Document;
use mongodb::Collection;

use crate::bindings::{
    CloseTab, CycleAppearance, DismissCommandPalette, OpenSettings, ToggleCommandPalette,
    ToggleSidebarRail,
};
use crate::command_palette::CommandPalette;
use crate::connection::registry::ConnectionRegistry;
use crate::connection::{AnyConnection, ConnectionEntry, ConnectionId, ConnectionState};
use crate::postgres;
use crate::project::{find_project_root, load_workspace_seed};
use crate::sqlite;

use object_info::ObjectInfoPanel;
use status_bar::StatusBar;
use topbar::Topbar;
use welcome::WelcomePanel;

/// Set dock tab label from [`TabSpec`] then register the panel.
macro_rules! register_dock_panel {
    ($ws:expr, $spec:expr, $ent:expr, $window:expr, $cx:expr) => {{
        let label = tab_label_for_spec(&$spec, false);
        $ent.update($cx, |panel, _| {
            panel.tab_label = label;
        });
        let arc = std::sync::Arc::new($ent);
        $ws.dock_add_and_register_tab($spec, arc, $window, $cx);
    }};
}

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

        workspace
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
                titlebar: Some(TitleBar::title_bar_options()),
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

    fn find_connection(
        &self,
        id: &ConnectionId,
        cx: &gpui::App,
    ) -> Option<Entity<ConnectionEntry>> {
        self.registry
            .read(cx)
            .connections()
            .iter()
            .find(|e| e.read(cx).id == *id)
            .cloned()
    }

    fn dispatch_open_tab(&mut self, spec: TabSpec, window: &mut Window, cx: &mut Context<Self>) {
        match spec {
            TabSpec::Dashboard(conn_id) => {
                self.connection_tree.update(cx, |tree, ecx| {
                    tree.focus_connection_by_id(&conn_id, ecx);
                });
            }
            TabSpec::DataViewer { conn_id, object } => {
                let Some(ent) = self.find_connection(&conn_id, cx) else {
                    return;
                };
                let ac = match &ent.read(cx).state {
                    ConnectionState::Connected(ac) => ac.clone(),
                    _ => return,
                };
                let tab_spec_for_manager = TabSpec::DataViewer {
                    conn_id: conn_id.clone(),
                    object: object.clone(),
                };
                match ac {
                    AnyConnection::SQLite(conn) => {
                        let pool = conn.read(cx).pool.clone();
                        let panel_ent = cx.new(|cx| {
                            sqlite::data_viewer::DataViewerPanel::new(pool, object, window, cx)
                        });
                        register_dock_panel!(self, tab_spec_for_manager, panel_ent, window, cx);
                    }
                    AnyConnection::Postgres(conn) => {
                        let pool = conn.read(cx).pool.clone();
                        let (schema, name) = match object.rsplit_once('.') {
                            Some((s, n)) if !n.is_empty() => (s.to_string(), n.to_string()),
                            _ => ("public".to_string(), object),
                        };
                        let panel_ent = cx.new(|cx| {
                            postgres::data_viewer::DataViewerPanel::new(
                                pool, schema, name, window, cx,
                            )
                        });
                        register_dock_panel!(self, tab_spec_for_manager, panel_ent, window, cx);
                    }
                    AnyConnection::MongoDB(conn) => {
                        let db = conn.read(cx).database().clone();
                        let collection: Collection<Document> = db.collection(&object);
                        let panel_ent = cx.new(|cx| {
                            crate::mongodb::document_viewer::DocumentViewerPanel::new(
                                collection, window, cx,
                            )
                        });
                        register_dock_panel!(self, tab_spec_for_manager, panel_ent, window, cx);
                    }
                }
            }
            TabSpec::QueryEditor {
                conn_id,
                initial_sql,
                initial_pipeline,
                auto_run,
                mongo_collection,
            } => {
                let Some(ent) = self.find_connection(&conn_id, cx) else {
                    return;
                };
                let ac = match &ent.read(cx).state {
                    ConnectionState::Connected(ac) => ac.clone(),
                    _ => return,
                };
                let tab_spec_for_manager = TabSpec::QueryEditor {
                    conn_id: conn_id.clone(),
                    initial_sql: initial_sql.clone(),
                    initial_pipeline: initial_pipeline.clone(),
                    auto_run,
                    mongo_collection: mongo_collection.clone(),
                };
                match ac {
                    AnyConnection::SQLite(conn) => {
                        let pool = conn.read(cx).pool.clone();
                        let panel_ent = cx.new(|cx| {
                            sqlite::query_editor::QueryEditorPanel::new_with_initial(
                                pool,
                                conn_id.clone(),
                                initial_sql.clone(),
                                auto_run,
                                window,
                                cx,
                            )
                        });
                        register_dock_panel!(self, tab_spec_for_manager, panel_ent, window, cx);
                    }
                    AnyConnection::Postgres(conn) => {
                        let pool = conn.read(cx).pool.clone();
                        let panel_ent = cx.new(|cx| {
                            postgres::query_editor::QueryEditorPanel::new_with_initial(
                                pool,
                                conn_id.clone(),
                                initial_sql.clone(),
                                auto_run,
                                window,
                                cx,
                            )
                        });
                        register_dock_panel!(self, tab_spec_for_manager, panel_ent, window, cx);
                    }
                    AnyConnection::MongoDB(conn) => {
                        let db = conn.read(cx).database().clone();
                        let coll_name = mongo_collection
                            .as_deref()
                            .map(str::trim)
                            .filter(|s| !s.is_empty())
                            .unwrap_or("based_explorer");
                        let coll: Collection<Document> = db.collection(coll_name);
                        let merged = initial_pipeline.clone().or(initial_sql.clone());
                        let panel_ent = cx.new(|cx| {
                            crate::mongodb::pipeline_builder::PipelineBuilderPanel::new_with_pipeline(
                                coll,
                                conn_id.clone(),
                                merged,
                                window,
                                cx,
                            )
                        });
                        register_dock_panel!(self, tab_spec_for_manager, panel_ent, window, cx);
                    }
                }
            }
            TabSpec::Pipeline {
                conn_id,
                collection,
            } => {
                let Some(ent) = self.find_connection(&conn_id, cx) else {
                    return;
                };
                let ac = match &ent.read(cx).state {
                    ConnectionState::Connected(ac) => ac.clone(),
                    _ => return,
                };
                let AnyConnection::MongoDB(conn) = ac else {
                    log::warn!("pipeline tab requires a MongoDB connection");
                    return;
                };
                let tab_spec_for_manager = TabSpec::Pipeline {
                    conn_id: conn_id.clone(),
                    collection: collection.clone(),
                };
                let db = conn.read(cx).database().clone();
                let coll: Collection<Document> = db.collection(&collection);
                let panel_ent = cx.new(|cx| {
                    crate::mongodb::pipeline_builder::PipelineBuilderPanel::new(
                        coll,
                        conn_id.clone(),
                        window,
                        cx,
                    )
                });
                register_dock_panel!(self, tab_spec_for_manager, panel_ent, window, cx);
            }
            TabSpec::Inspector { conn_id, object } => {
                let Some(ent) = self.find_connection(&conn_id, cx) else {
                    return;
                };
                let ac = match &ent.read(cx).state {
                    ConnectionState::Connected(ac) => ac.clone(),
                    _ => return,
                };
                let tab_spec_for_manager = TabSpec::Inspector {
                    conn_id: conn_id.clone(),
                    object: object.clone(),
                };
                match ac {
                    AnyConnection::SQLite(conn) => {
                        let pool = conn.read(cx).pool.clone();
                        let panel_ent = cx.new(|cx| {
                            sqlite::inspector::TableInspectorPanel::new(
                                pool,
                                object.clone(),
                                window,
                                cx,
                            )
                        });
                        register_dock_panel!(self, tab_spec_for_manager, panel_ent, window, cx);
                    }
                    AnyConnection::Postgres(conn) => {
                        let pool = conn.read(cx).pool.clone();
                        let (schema, name) = match object.rsplit_once('.') {
                            Some((s, n)) if !n.is_empty() => (s.to_string(), n.to_string()),
                            _ => ("public".to_string(), object.clone()),
                        };
                        let panel_ent = cx.new(|cx| {
                            postgres::inspector::TableInspectorPanel::new(
                                pool, schema, name, window, cx,
                            )
                        });
                        register_dock_panel!(self, tab_spec_for_manager, panel_ent, window, cx);
                    }
                    AnyConnection::MongoDB(conn) => {
                        let db = conn.read(cx).database().clone();
                        let coll: Collection<Document> = db.collection(&object);
                        let panel_ent = cx.new(|cx| {
                            crate::mongodb::inspector::CollectionInspectorPanel::new(
                                coll, window, cx,
                            )
                        });
                        register_dock_panel!(self, tab_spec_for_manager, panel_ent, window, cx);
                    }
                }
            }
            TabSpec::ObjectInfo {
                conn_id,
                object_name,
                kind_label,
            } => {
                let tab_spec_for_manager = TabSpec::ObjectInfo {
                    conn_id: conn_id.clone(),
                    object_name: object_name.clone(),
                    kind_label: kind_label.clone(),
                };
                let panel_ent = cx.new(|cx| {
                    ObjectInfoPanel::new(object_name.clone(), kind_label.clone(), window, cx)
                });
                register_dock_panel!(self, tab_spec_for_manager, panel_ent, window, cx);
            }
            TabSpec::DocumentInsert {
                conn_id,
                collection,
            } => {
                let Some(ent) = self.find_connection(&conn_id, cx) else {
                    return;
                };
                let ac = match &ent.read(cx).state {
                    ConnectionState::Connected(ac) => ac.clone(),
                    _ => return,
                };
                let AnyConnection::MongoDB(conn) = ac else {
                    log::warn!("document insert tab requires MongoDB");
                    return;
                };
                let tab_spec_for_manager = TabSpec::DocumentInsert {
                    conn_id: conn_id.clone(),
                    collection: collection.clone(),
                };
                let db = conn.read(cx).database().clone();
                let coll: Collection<Document> = db.collection(&collection);
                let panel_ent = cx.new(|cx| {
                    crate::mongodb::document_editor::DocumentEditorPanel::new_insert(
                        coll, window, cx,
                    )
                });
                register_dock_panel!(self, tab_spec_for_manager, panel_ent, window, cx);
            }
            TabSpec::Explain {
                conn_id,
                label: _,
                sql,
            } => {
                let Some(ent) = self.find_connection(&conn_id, cx) else {
                    return;
                };
                let ac = match &ent.read(cx).state {
                    ConnectionState::Connected(ac) => ac.clone(),
                    _ => return,
                };
                let tab_spec_for_manager = TabSpec::Explain {
                    conn_id: conn_id.clone(),
                    label: "explain".to_string(),
                    sql: sql.clone(),
                };
                match ac {
                    AnyConnection::Postgres(conn) => {
                        let pool = conn.read(cx).pool.clone();
                        let panel_ent = cx
                            .new(|cx| postgres::explain::ExplainPanel::new(pool, sql, window, cx));
                        register_dock_panel!(self, tab_spec_for_manager, panel_ent, window, cx);
                    }
                    AnyConnection::SQLite(conn) => {
                        let pool = conn.read(cx).pool.clone();
                        let panel_ent = cx.new(|cx| {
                            sqlite::eqp_viewer::EqpViewerPanel::new(pool, sql, window, cx)
                        });
                        register_dock_panel!(self, tab_spec_for_manager, panel_ent, window, cx);
                    }
                    AnyConnection::MongoDB(_) => {}
                }
            }
        }
    }

    fn dock_add_and_register_tab(
        &mut self,
        spec: TabSpec,
        panel: Arc<dyn gpui_component::dock::PanelView>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.dock_area.update(cx, |dock, ecx| {
            dock.add_panel(panel.clone(), DockPlacement::Center, None, window, ecx);
        });
        let view: AnyView = panel.as_ref().into();
        self.tab_manager.update(cx, |tm, ecx| {
            tm.open_or_focus(spec, view, ecx);
        });
        cx.notify();
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
            .child(self.command_palette.clone())
    }
}

fn center_tab_items(item: &DockItem) -> Option<&[Arc<dyn PanelView>]> {
    match item {
        DockItem::Tabs { items, .. } => Some(items),
        DockItem::Split { items, .. } => items.iter().find_map(center_tab_items),
        _ => None,
    }
}

fn active_center_tab(item: &DockItem, _cx: &App) -> Option<(Arc<dyn PanelView>, usize)> {
    match item {
        DockItem::Tabs {
            items, active_ix, ..
        } => {
            let panel = items.get(*active_ix)?.clone();
            Some((panel, items.len()))
        }
        DockItem::Split { items, .. } => {
            items.iter().find_map(|child| active_center_tab(child, _cx))
        }
        _ => None,
    }
}

fn collect_dock_panel_views(item: &DockItem, out: &mut Vec<AnyView>) {
    match item {
        DockItem::Split { items, .. } => {
            for child in items {
                collect_dock_panel_views(child, out);
            }
        }
        DockItem::Tabs { items, .. } => {
            for p in items {
                out.push(p.view());
            }
        }
        DockItem::Panel { view, .. } => {
            out.push(view.view());
        }
        DockItem::Tiles { .. } => {}
    }
}

fn dock_area_present_views(dock: &DockArea, cx: &App) -> Vec<AnyView> {
    let mut v = Vec::new();
    collect_dock_panel_views(dock.center(), &mut v);
    if let Some(left) = dock.left_dock() {
        collect_dock_panel_views(left.read(cx).panel(), &mut v);
    }
    if let Some(right) = dock.right_dock() {
        collect_dock_panel_views(right.read(cx).panel(), &mut v);
    }
    if let Some(bottom) = dock.bottom_dock() {
        collect_dock_panel_views(bottom.read(cx).panel(), &mut v);
    }
    v
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
        .rounded(gpui::px(crate::widgets::ui::PANEL_RADIUS))
        .border_1()
        .border_color(cx.theme().border.opacity(0.85))
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
                .gap_2()
                .border_b_1()
                .border_color(cx.theme().border.opacity(0.42))
                .child(
                    div()
                        .w(gpui::px(88.0))
                        .flex_shrink_0()
                        .text_xs()
                        .text_color(cx.theme().muted_foreground)
                        .child(label),
                )
                .child(
                    div()
                        .flex_1()
                        .min_w_0()
                        .text_xs()
                        .text_color(cx.theme().foreground.opacity(0.88))
                        .truncate()
                        .child(value),
                )
        }))
}
