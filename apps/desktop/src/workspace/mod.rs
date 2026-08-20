// workspace/ — Workspace entity, DockArea, tabs, connection tree; shell chrome in `chrome/`.

pub mod chrome;
pub mod panels;
pub mod pop_out;
pub mod tabs;

pub use pop_out::PopOutManager;
pub use tabs::{
    DockAreaRef, QueryEditorInit, SqlInject, TabManager, TabManagerRef, TabOpenQueue, TabSpec,
    WorkspaceNavQueue, WorkspaceRef, enqueue_sql_inject, mark_query_tab_dirty,
};

pub mod connection_tree;
pub use connection_tree::ConnectionTree;

pub mod connection_destination;
pub mod connection_persist;
pub mod context;
pub mod item;
pub mod notify;
pub mod project_query;
pub mod query_lane;
pub mod templates;
pub mod wizard_logic;

mod center_panels;
mod dock_utils;
mod pending_ops;
mod project_integration;
mod render;

use std::path::PathBuf;
use std::slice;
use std::sync::Arc;

use dock_utils::tabs_layout;

use gpui::{App, Context, Entity, FocusHandle, Focusable, SharedString, Window, prelude::*};
use gpui_component::dock::{DockArea, DockEvent, DockSkin, PanelStyle, PanelView};

use crate::app::prefs::{collapsed_from, set_sidebar};
use crate::app::quit::confirm_before_close_window;
use crate::app::shell::open_settings;
use crate::command_palette::{
    CommandPalette,
    PaletteEvent::{InjectSql, OpenProjectQuery, OpenTab, WorkspaceAction},
};
use crate::connection::ConnectionId;
use crate::connection::ConnectionOrigin;
use crate::connection::registry::ConnectionRegistry;
use based_project::ProjectQuery;

use crate::project::{
    ProjectContext, ProjectRoot, RegistryRef, find_project_root,
    loader::load_entries_from_based_dir, personal::personal_root,
};

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
    /// Queued Close Project; confirm dialog on next [`Render`].
    pub(crate) pending_project_close_confirm: bool,
    tab_navigation: TabNavigationHistory,
    /// Live center tab panels, kept as styled handles for TabManager downcasts.
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
            let personal =
                load_entries_from_based_dir(&personal_root(), ConnectionOrigin::Personal);
            reg.sync_origin_entries(ConnectionOrigin::Personal, personal, cx);
            if let Some(root) = project_dir.as_ref() {
                let project =
                    load_entries_from_based_dir(&root.join(".based"), ConnectionOrigin::Project);
                reg.sync_origin_entries(ConnectionOrigin::Project, project, cx);
            }
        });

        if registry.read(cx).connections().is_empty() {
            log::info!("no connections loaded; open a folder with .based/connections/");
        }

        let (dock_area, skin) = DockSkin::dock_area("workspace", Some(1), window, cx);
        skin.set_panel_style(PanelStyle::TabBar, cx);

        let home = cx.new(|cx| HomePanel::new(window, cx));
        let home_panel = home.clone();
        let home_arc: Arc<dyn PanelView> = Arc::new(home.clone());
        dock_area.update(cx, |area, cx| {
            area.set_center(tabs_layout(slice::from_ref(&home_arc), cx), window, cx);
        });

        let connection_tree =
            cx.new(|cx| ConnectionTree::new(registry.clone(), dock_area.clone(), cx));

        let tab_manager = cx.new(|_| TabManager::new());
        cx.set_global(TabManagerRef(tab_manager.clone()));
        cx.set_global(DockAreaRef(dock_area.clone()));
        if let Some(root) = project_dir.clone() {
            cx.set_global(RegistryRef(registry.clone()));
            cx.set_global(ProjectRoot(root));
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
            sidebar_collapsed: collapsed_from(cx),
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
            pending_project_close_confirm: false,
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
                OpenTab(spec) => {
                    ws.pending_open_tab = Some(spec.clone());
                }
                InjectSql { conn_id, sql } => {
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
                            init: QueryEditorInit::Sql {
                                sql: Some(sql.clone()),
                                auto_run: false,
                            },
                        });
                    }
                }
                OpenProjectQuery(path) => {
                    ws.open_project_query_by_path(path, ecx);
                }
                WorkspaceAction(action) => {
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
            let result =
                confirm_before_close_window(&registry_for_close, &workspace_for_close, window, cx);
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
        let mut tabs = Vec::new();
        let mut active = None;
        for (i, t) in tm.tabs.iter().enumerate() {
            if !t.spec.persist_in_session() {
                continue;
            }
            if tm.active_idx == Some(i) {
                active = Some(tabs.len());
            }
            tabs.push(t.spec.clone());
        }
        let snapshot = tabs::SessionSnapshot {
            tabs,
            active,
            active_connection_id: self.focused_conn_id(cx).map(|id| id.0.clone()),
            pinned_tabs: tm
                .pinned_specs()
                .into_iter()
                .filter(TabSpec::persist_in_session)
                .collect(),
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

        let active_spec = session
            .active
            .and_then(|idx| session.tabs.get(idx).cloned());
        for spec in session.tabs {
            if matches!(spec, TabSpec::Home) || !spec.persist_in_session() {
                continue;
            }
            self.pending_open_tab = Some(spec);
            self.flush_pending_open_tab(window, cx);
        }
        if let Some(spec) = active_spec.filter(TabSpec::persist_in_session) {
            self.tab_manager.update(cx, |tm, ecx| {
                if let Some(idx) = tm.tabs.iter().position(|t| t.spec == spec) {
                    tm.activate(idx, ecx);
                }
            });
        }
        let pinned: Vec<TabSpec> = session
            .pinned_tabs
            .into_iter()
            .filter(TabSpec::persist_in_session)
            .collect();
        if !pinned.is_empty() {
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

    pub fn has_dirty_project_tabs(&self, cx: &App) -> bool {
        self.tab_manager
            .read(cx)
            .tabs
            .iter()
            .any(|t| t.dirty && t.spec.conn_id().is_some_and(|id| !id.is_workspace_local()))
    }

    pub fn toggle_sidebar_rail(&mut self, cx: &mut Context<Self>) {
        self.sidebar_collapsed = !self.sidebar_collapsed;
        set_sidebar(self.sidebar_collapsed, cx);
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
            set_sidebar(false, cx);
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
            .and_then(|t| t.spec.conn_id().cloned())
    }

    /// Thin shim retained so the existing `OpenSettings` action listener
    /// (registered with `window.listener_for(&this, ...)`) and any in-window
    /// callers keep compiling. The real work lives in
    /// [`crate::app::shell::open_settings`].
    pub fn open_settings(&mut self, _window: &mut Window, cx: &mut Context<Self>) {
        open_settings(&mut *cx);
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
