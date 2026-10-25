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

pub mod boot;
pub mod connection_destination;
pub mod connection_persist;
pub mod context;
pub mod notify;
pub mod project_query;
pub mod query_lane;
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

use crate::ai::OFF_NOTICE;
use crate::ai::chat::{ChatContext, ChatPane};
use based_ai::context::ContextLevel;
use gpui_kit::component::dock::{DockArea, DockEvent, DockSkin, PanelStyle, PanelView};
use gpui_kit::{App, Context, Entity, FocusHandle, Focusable, SharedString, Window, prelude::*};

use crate::app::paths::AppPaths;
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
use crate::db::{self, Tokio};
use based_project::ProjectQuery;

use crate::project::{
    ProjectContext, ProjectRoot, RegistryRef, loader::load_entries_from_based_dir,
};

use crate::storage;
use crate::widgets::query_panel_extras::HistoryFilter;
use boot::WorkspaceEnv;
use context::WorkspaceContext;

use tabs::TabNavigationHistory;

use chrome::{
    left_pane::LeftPane,
    side_pane::{SIDE_PANE_WIDTH, SidePane},
};
use panels::HomePanel;

/// See [`Workspace::chat_conn`].
struct ChatScope {
    conn: ConnectionId,
    focused: Option<ConnectionId>,
}

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
    /// Width of the right pane; dragged by its left edge.
    side_pane_width: f32,
    /// History pane filter chip (All / Today).
    history_filter: HistoryFilter,
    chat: Entity<ChatPane>,
    /// Connection an explicit ask pinned the assistant to, valid while the same tab is focused.
    chat_scope: Option<ChatScope>,
    /// Last connection a focused tab belonged to; the assistant keeps it on Home/non-connection tabs.
    last_chat_conn: Option<ConnectionId>,
    focus_handle: FocusHandle,
    project_title: SharedString,
    project_dir: Option<PathBuf>,
    session: tabs::SessionState,
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
    /// Close request waiting on a discard-edits confirmation.
    close_prompt: Option<tabs::ClosePrompt>,
    /// Live center tab panels, kept as styled handles for TabManager downcasts.
    center_panels: Vec<Arc<dyn PanelView>>,
}

impl Workspace {
    pub fn new(window: &mut Window, cx: &mut Context<Self>) -> Self {
        Self::with_env(WorkspaceEnv::from_paths(AppPaths::get(cx)), window, cx)
    }

    pub fn with_env(env: WorkspaceEnv, window: &mut Window, cx: &mut Context<Self>) -> Self {
        let WorkspaceEnv {
            project_dir,
            personal_root,
        } = env;
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
            let personal = load_entries_from_based_dir(&personal_root, ConnectionOrigin::Personal);
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

        let tab_manager = cx.new(|_| TabManager::default());
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
            side_pane_width: SIDE_PANE_WIDTH,
            history_filter: HistoryFilter::default(),
            chat: cx.new(|cx| ChatPane::new(registry.clone(), window, cx)),
            chat_scope: None,
            last_chat_conn: None,
            focus_handle: cx.focus_handle(),
            project_title,
            project_dir,
            session: tabs::SessionState::default(),
            pending_open_tab: None,
            pending_target_pick: None,
            pending_close_confirm: false,
            pending_project_switch: None,
            pending_project_switch_confirm: false,
            pending_project_close_confirm: false,
            tab_navigation: TabNavigationHistory::default(),
            close_prompt: None,
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

    pub fn tab_manager(&self) -> &Entity<TabManager> {
        &self.tab_manager
    }

    pub fn connection_tree(&self) -> &Entity<ConnectionTree> {
        &self.connection_tree
    }

    pub fn command_palette(&self) -> &Entity<CommandPalette> {
        &self.command_palette
    }

    fn save_session(&mut self, cx: &mut Context<Self>) {
        let active_conn = self.focused_conn_id(cx).map(|id| id.0.clone());
        let tm = self.tab_manager.read(cx);
        let Some(snapshot) =
            tabs::SessionSnapshot::capture(tm, self.session.pending(), active_conn)
        else {
            return;
        };
        if !self.session.enqueue(snapshot) {
            return;
        }
        let store = storage::store(cx);
        cx.spawn(async move |this, cx| {
            // Coalesce: keep writing until no newer snapshot arrived during the last write.
            while let Ok(Some(snapshot)) = this.update(cx, |ws, _| ws.session.take_queued()) {
                let store = store.clone();
                let saved = db::run(cx, async move { snapshot.save(&store).await }).await;
                if let Err(err) = saved {
                    log::warn!("session save failed: {err:#}");
                }
            }
            let _ = this.update(cx, |ws, _| ws.session.finish_writing());
        })
        .detach();
    }

    fn restore_session(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let store = storage::store(cx);
        let handle = Tokio::handle(cx);
        let session = handle.block_on(tabs::SessionSnapshot::load(&store));

        let active_spec = session
            .active
            .and_then(|idx| session.tabs.get(idx).cloned());
        self.session.set_pending(session.tabs);
        self.flush_pending_session_tabs(window, cx);
        if let Some(spec) = active_spec.filter(TabSpec::persist_in_session) {
            self.focus_existing_tab(&spec, window, cx);
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

    /// Toggles the assistant pane and focuses its composer when it opens.
    pub fn toggle_chat(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.toggle_side_pane(SidePane::Chat, cx);
        if self.active_side_pane == Some(SidePane::Chat) {
            self.chat.update(cx, |chat, cx| chat.focus(window, cx));
        }
    }

    /// Shows the assistant (never hides it) and focuses its input.
    pub fn open_chat(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if self.active_side_pane != Some(SidePane::Chat) {
            self.toggle_side_pane(SidePane::Chat, cx);
        }
        self.chat.update(cx, |chat, cx| chat.focus(window, cx));
    }

    /// Connection the assistant talks about: an explicit ask, else the focused tab's.
    fn chat_conn(&mut self, cx: &App) -> Option<ConnectionId> {
        let focused = self.focused_conn_id(cx);
        if let Some(scope) = &self.chat_scope {
            if scope.focused == focused {
                return Some(scope.conn.clone());
            }
            self.chat_scope = None;
        }
        if focused.is_some() {
            self.last_chat_conn.clone_from(&focused);
        }
        focused.or_else(|| self.last_chat_conn.clone())
    }

    /// Points the assistant at its connection (and gates it on that connection's AI setting).
    fn sync_chat_context(&mut self, cx: &mut Context<Self>) {
        let entry = self
            .chat_conn(cx)
            .and_then(|id| self.registry.read(cx).get(&id, cx).cloned());
        let (ctx, notice) = match entry.as_ref().map(|e| (e, e.read(cx))) {
            Some((entity, _)) => {
                let ctx = ChatContext::of(entity, cx);
                let level = ctx.level;
                let notice = (level == ContextLevel::Off).then(|| SharedString::from(OFF_NOTICE));
                (Some(ctx), notice)
            }
            None => (None, None),
        };
        self.chat
            .update(cx, |chat, cx| chat.set_context(ctx, notice, cx));
    }

    /// Opens the assistant about `conn` and sends `text`.
    pub fn ask_assistant(&mut self, conn: &ConnectionId, text: String, cx: &mut Context<Self>) {
        self.chat_scope = Some(ChatScope {
            conn: conn.clone(),
            focused: self.focused_conn_id(cx),
        });
        self.active_side_pane = Some(SidePane::Chat);
        self.sync_chat_context(cx);
        self.chat.update(cx, |chat, cx| {
            chat.ask(text, cx);
        });
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
    fn focus_handle(&self, _: &gpui_kit::App) -> FocusHandle {
        self.focus_handle.clone()
    }
}
