// workspace/ — Workspace entity, DockArea, tabs, connection tree; shell chrome in `chrome/`.

pub mod chrome;
pub mod panels;
pub mod pop_out;
pub mod tabs;

pub use pop_out::PopOutManager;
pub use tabs::{
    DockAreaRef, SqlInject, TabManager, TabManagerRef, TabOpenQueue, TabSpec, WorkspaceNavQueue,
    WorkspaceRef, enqueue_sql_inject, mark_query_tab_dirty,
};

pub mod connection_tree;
pub use connection_tree::ConnectionTree;

pub mod context;
pub mod item;
pub mod notify;
pub mod project_query;
pub mod query_lane;
pub mod templates;

mod center_panels;
mod dock_utils;
mod pending_ops;
mod project_integration;
mod render;

use std::sync::Arc;

use dock_utils::wrap_center_root;

use std::path::PathBuf;

use gpui::{App, Context, Entity, FocusHandle, Focusable, SharedString, Window, prelude::*};
use gpui_component::dock::{DockArea, DockEvent, DockItem, PanelStyle, PanelView};

use crate::command_palette::CommandPalette;
use crate::connection::ConnectionId;
use crate::connection::registry::ConnectionRegistry;
use based_project::ProjectQuery;

use crate::project::{ProjectContext, find_project_root, loader::entry_from_project};

use crate::storage;
use crate::widgets::query_panel_extras::HistoryFilter;
use context::WorkspaceContext;

use tabs::TabNavigationHistory;

use chrome::{left_pane::LeftPane, side_pane::SidePane};
use panels::HomePanel;

pub struct Workspace {
    registry: Entity<ConnectionRegistry>,
    home_panel: Entity<HomePanel>,
    dock_area: Entity<DockArea>,
    connection_tree: Entity<ConnectionTree>,
    tab_manager: Entity<TabManager>,
    command_palette: Entity<CommandPalette>,
    sidebar_collapsed: bool,
    active_left_pane: LeftPane,
    /// `None` collapses the right-hand column. Defaults to Inspector to preserve the prior UX.
    active_side_pane: Option<SidePane>,
    /// History pane filter chip (All / Today).
    history_filter: HistoryFilter,
    focus_handle: FocusHandle,
    project_title: SharedString,
    project_dir: Option<PathBuf>,
    session_restored: bool,
    pending_open_tab: Option<TabSpec>,
    pending_target_pick: Option<(ProjectQuery, Vec<ConnectionId>)>,
    /// Set by platform close; dialog is shown on the next [`Render`] (see `app::quit`).
    pub(crate) pending_close_confirm: bool,
    /// Queued in-place project switch; confirm dialog on next [`Render`].
    pub(crate) pending_project_switch: Option<PathBuf>,
    pub(crate) pending_project_switch_confirm: bool,
    tab_navigation: TabNavigationHistory,
    /// Live center tab panels. gpui-component `DockItem.items` can desync from `TabPanel.panels`
    /// (split add and tab remove do not always update the snapshot `items` vec).
    center_panels: Vec<Arc<dyn PanelView>>,
}

impl Workspace {
    pub fn new(window: &mut Window, cx: &mut Context<Self>) -> Self {
        let project_dir = find_project_root();
        let project_context = project_dir
            .as_ref()
            .and_then(|root| ProjectContext::load(root.clone()).ok());

        let workspace_ctx = WorkspaceContext::load_initial(cx).unwrap_or_else(|e| {
            log::error!("workspace context load failed: {e:#}");
            WorkspaceContext {
                active: based_workspace::WorkspaceModel::new("Default"),
                summaries: vec![],
            }
        });
        cx.set_global(workspace_ctx.clone());

        let project_title: SharedString = project_context
            .as_ref()
            .map(|c| c.project_name().into())
            .unwrap_or_else(|| workspace_ctx.active.name.clone().into());

        let registry = cx.new(ConnectionRegistry::new);
        registry.update(cx, |reg, cx| {
            if let Some(ref ctx) = project_context {
                let mut entries = Vec::new();
                for conn in &ctx.snapshot.connections {
                    match entry_from_project(conn) {
                        Ok(e) => entries.push(e),
                        Err(e) => log::warn!("connection {} skipped: {e:#}", conn.id),
                    }
                }
                reg.sync_project_entries(entries, cx);
            }
        });

        if registry.read(cx).connections().is_empty() {
            log::info!("no connections loaded; open a folder with .based/connections/");
        }

        let dock_area = cx.new(|cx| {
            DockArea::new("workspace", Some(1), window, cx).panel_style(PanelStyle::TabBar)
        });

        let home = cx.new(|cx| HomePanel::new(window, cx));
        let home_panel = home.clone();
        let home_arc: Arc<dyn PanelView> = Arc::new(home.clone());
        let weak_dock = dock_area.downgrade();
        let tabs = DockItem::tab(home, &weak_dock, window, cx);
        let center = wrap_center_root(tabs, &weak_dock, window, cx);
        dock_area.update(cx, |area, cx| {
            area.set_center(center, window, cx);
        });

        let connection_tree =
            cx.new(|cx| ConnectionTree::new(registry.clone(), dock_area.clone(), cx));

        let tab_manager = cx.new(|_| TabManager::new());
        cx.set_global(TabManagerRef(tab_manager.clone()));
        cx.set_global(DockAreaRef(dock_area.clone()));
        if let Some(root) = project_dir.clone() {
            cx.set_global(crate::project::RegistryRef(registry.clone()));
            cx.set_global(crate::project::ProjectRoot(root));
        }
        let command_palette =
            cx.new(|cx| CommandPalette::new(registry.clone(), connection_tree.clone(), window, cx));
        let palette_observe = command_palette.clone();

        let workspace_options: Vec<SharedString> = workspace_ctx
            .workspace_options()
            .into_iter()
            .map(SharedString::from)
            .collect();
        let _workspace_options = workspace_options;

        let tree_observe = connection_tree.clone();

        let workspace = Self {
            registry: registry.clone(),
            home_panel,
            dock_area,
            connection_tree,
            tab_manager,
            command_palette,
            sidebar_collapsed: crate::app::prefs::collapsed_from(cx),
            active_left_pane: LeftPane::Browser,
            active_side_pane: None,
            history_filter: HistoryFilter::default(),
            focus_handle: cx.focus_handle(),
            project_title,
            project_dir,
            session_restored: false,
            pending_open_tab: None,
            pending_target_pick: None,
            pending_close_confirm: false,
            pending_project_switch: None,
            pending_project_switch_confirm: false,
            tab_navigation: TabNavigationHistory::default(),
            center_panels: vec![home_arc],
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
                crate::command_palette::PaletteEvent::OpenProjectQuery(path) => {
                    ws.open_project_query_by_path(path, ecx);
                }
                crate::command_palette::PaletteEvent::WorkspaceAction(action) => {
                    ws.handle_palette_workspace_action(action.clone(), ecx);
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
        cx.subscribe(&tab_mgr_observe, |ws, _, _: &tabs::TabEvent, ecx| {
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
        let tm = self.tab_manager.read(cx);
        if tm.tabs.is_empty() {
            return;
        }
        let snapshot = tabs::SessionSnapshot {
            tabs: tm.tabs.iter().map(|t| t.spec.clone()).collect(),
            active: tm.active_idx,
            active_connection_id: self.focused_conn_id(cx).map(|id| id.0.clone()),
            pinned_tabs: tm.pinned_specs(),
        };
        let store = storage::store(cx);
        let handle = gpui_tokio::Tokio::handle(cx);
        if let Err(err) = handle.block_on(snapshot.save(&store)) {
            log::warn!("session save failed: {err:#}");
        }
    }

    fn restore_session(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let store = storage::store(cx);
        let handle = gpui_tokio::Tokio::handle(cx);
        let session = handle.block_on(tabs::SessionSnapshot::load(&store));

        if let Some(conn_key) = session.active_connection_id.clone() {
            let conn_id = ConnectionId(conn_key);
            self.connection_tree.update(cx, |tree, ecx| {
                tree.focus_connection_by_id(&conn_id, ecx);
            });
        }

        for spec in session.tabs {
            if matches!(spec, TabSpec::Home) {
                continue;
            }
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
        if !session.pinned_tabs.is_empty() {
            let pinned = session.pinned_tabs;
            self.sync_tab_manager_from_dock(cx);
            self.tab_manager.update(cx, |tm, ecx| {
                tm.apply_pinned_specs(&pinned, ecx);
            });
            self.refresh_tab_strip_chrome(cx);
        } else {
            self.sync_tab_manager_from_dock(cx);
        }
    }

    pub fn has_dirty_tabs(&self, cx: &App) -> bool {
        self.tab_manager.read(cx).tabs.iter().any(|t| t.dirty)
    }

    pub fn toggle_sidebar_rail(&mut self, cx: &mut Context<Self>) {
        self.sidebar_collapsed = !self.sidebar_collapsed;
        crate::app::prefs::set_sidebar(self.sidebar_collapsed, cx);
        cx.notify();
    }

    pub fn toggle_left_pane(&mut self, pane: LeftPane, cx: &mut Context<Self>) {
        if self.active_left_pane == pane && !self.sidebar_collapsed {
            self.toggle_sidebar_rail(cx);
            return;
        }
        self.active_left_pane = pane;
        if self.sidebar_collapsed {
            self.sidebar_collapsed = false;
            crate::app::prefs::set_sidebar(false, cx);
        }
        cx.notify();
    }

    /// Click a rail icon: switch to that pane, or collapse if it was already active.
    pub fn toggle_side_pane(&mut self, pane: SidePane, cx: &mut Context<Self>) {
        self.active_side_pane = if self.active_side_pane == Some(pane) {
            None
        } else {
            Some(pane)
        };
        cx.notify();
    }

    pub fn set_history_filter(&mut self, filter: HistoryFilter, cx: &mut Context<Self>) {
        self.history_filter = filter;
        cx.notify();
    }

    /// Connection id of the currently focused center tab (used to scope History/Saved panes).
    pub fn focused_conn_id(&self, cx: &App) -> Option<ConnectionId> {
        self.tab_manager
            .read(cx)
            .active_tab()
            .map(|t| t.spec.conn_id().clone())
    }

    /// Thin shim retained so the existing `OpenSettings` action listener
    /// (registered with `window.listener_for(&this, ...)`) and any in-window
    /// callers keep compiling. The real work lives in
    /// [`crate::app::shell::open_settings`].
    pub fn open_settings(&mut self, _window: &mut Window, cx: &mut Context<Self>) {
        crate::app::shell::open_settings(&mut *cx);
    }

    pub fn toggle_command_palette(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.command_palette
            .update(cx, |p, cx| p.toggle(window, cx));
    }
}

impl Focusable for Workspace {
    fn focus_handle(&self, _: &gpui::App) -> FocusHandle {
        self.focus_handle.clone()
    }
}
