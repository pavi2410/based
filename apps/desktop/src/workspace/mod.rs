// workspace/ — Workspace entity, DockArea, tabs, connection tree; shell chrome in `chrome/`.

pub mod chrome;
pub mod panels;
pub mod pop_out;
pub mod tabs;

pub use pop_out::PopOutManager;
pub use tabs::{Inbox, QueryEditorInit, TabManager, TabSpec, Workspaces};

pub mod connection_tree;
pub use connection_tree::ConnectionTree;

pub mod boot;
pub mod connection_destination;
pub mod connection_health;
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
use std::time::Duration;

use dock_utils::tabs_layout;

use crate::ai::OFF_NOTICE;
use crate::ai::chat::{ChatContext, ChatPane};
use based_ai::context::ContextLevel;
use gpui_kit::component::WindowExt as _;
use gpui_kit::component::dock::{DockArea, DockEvent, DockSkin, PanelStyle, PanelView};
use gpui_kit::{
    App, Context, Entity, FocusHandle, Focusable, SharedString, Task, Window, WindowId, prelude::*,
};

use crate::app::paths::AppPaths;
use crate::app::prefs::{collapsed_from, set_sidebar};
use crate::app::quit::confirm_before_close_window;
use crate::app::quit::show_queued_quit;
use crate::app::shell::open_settings;
use crate::command_palette::{
    CommandPalette,
    PaletteEvent::{InjectSql, OpenProjectQuery, OpenTab, WorkspaceAction},
};
use crate::connection::ConnectionId;
use crate::connection::ConnectionOrigin;
use crate::connection::registry::ConnectionRegistry;
use crate::db::{self, Tokio};
use crate::project::open::{show_queued_close, show_queued_switch};
use based_project::ProjectQuery;

use crate::project::{
    ProjectState, loader::load_entries_from_based_dir, settings::apply_project_settings,
};

use crate::storage;
use crate::widgets::query_panel_extras::HistoryFilter;
use boot::WorkspaceEnv;
use context::WorkspaceContext;

use tabs::{TabNavigationHistory, inbox};

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

const SESSION_EDIT_DEBOUNCE: Duration = Duration::from_secs(2);

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
    /// The window this workspace fills; its key in [`Workspaces`].
    window_id: WindowId,
    session: tabs::SessionState,
    pending_target_pick: Option<(ProjectQuery, Vec<ConnectionId>)>,
    /// Debounced session save after editor typing (see [`Self::note_editor_edit`]).
    session_edit_save: Option<Task<()>>,
    /// A confirm dialog to show on the next [`Render`] (see [`QueuedDialog`]).
    queued_dialog: Option<QueuedDialog>,
    tab_navigation: TabNavigationHistory,
    /// Close request waiting on a discard-edits confirmation.
    close_prompt: Option<tabs::ClosePrompt>,
}

/// A confirm dialog requested outside render (platform close, menus, the
/// palette) and shown on the next frame, where a window is at hand.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum QueuedDialog {
    /// Close the window (quit flow, see `app::quit`).
    Quit,
    /// Switch this window to the project at the path.
    SwitchProject(PathBuf),
    CloseProject,
}

/// Which request to keep when another arrives: quitting wins; otherwise the
/// newest request replaces the older one.
fn merge_queued(current: Option<QueuedDialog>, new: QueuedDialog) -> QueuedDialog {
    match current {
        Some(QueuedDialog::Quit) => QueuedDialog::Quit,
        _ => new,
    }
}

impl Workspace {
    pub(crate) fn queue_dialog(&mut self, dialog: QueuedDialog) {
        self.queued_dialog = Some(merge_queued(self.queued_dialog.take(), dialog));
    }

    /// Show the queued dialog, if any. A project switch or close stays queued
    /// while another dialog is open; a quit request is dropped then (see `app::quit`).
    pub(crate) fn show_queued_dialog(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let Some(dialog) = self.queued_dialog.take() else {
            return;
        };
        let shown = match &dialog {
            QueuedDialog::Quit => show_queued_quit(self, window, cx),
            QueuedDialog::SwitchProject(root) => show_queued_switch(self, root.clone(), window, cx),
            QueuedDialog::CloseProject => show_queued_close(self, window, cx),
        };
        if !shown {
            self.queue_dialog(dialog);
        }
    }

    pub fn new(window: &mut Window, cx: &mut Context<Self>) -> Self {
        Self::with_env(WorkspaceEnv::from_paths(AppPaths::get(cx)), window, cx)
    }

    pub fn with_env(env: WorkspaceEnv, window: &mut Window, cx: &mut Context<Self>) -> Self {
        let WorkspaceEnv {
            project_dir,
            personal_root,
        } = env;
        let project = project_dir
            .clone()
            .map_or_else(ProjectState::none, ProjectState::open);
        if let Some(ctx) = project.context() {
            apply_project_settings(&ctx.snapshot.manifest, cx);
        }
        let project_context = project.context().cloned();

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
        let mut skipped = Vec::new();
        registry.update(cx, |reg, cx| {
            let personal = load_entries_from_based_dir(&personal_root, ConnectionOrigin::Personal);
            reg.sync_origin_entries(ConnectionOrigin::Personal, personal.entries, cx);
            skipped.push((personal_root.clone(), personal.skipped));
            if let Some(root) = project_dir.as_ref() {
                let based_dir = root.join(".based");
                let project = load_entries_from_based_dir(&based_dir, ConnectionOrigin::Project);
                reg.sync_origin_entries(ConnectionOrigin::Project, project.entries, cx);
                skipped.push((based_dir, project.skipped));
            }
        });
        let skipped_queries = project_context.as_ref().map(|ctx| {
            (
                ctx.root.join(".based"),
                ctx.snapshot.skipped_queries.clone(),
            )
        });
        // After the first frame, so the notice lands on this window's tray.
        cx.defer_in(window, move |_, window, cx| {
            for (based_dir, skipped) in skipped {
                if let Some(note) = notify::skipped_connections_notice(cx, &based_dir, &skipped) {
                    window.push_notification(note, cx);
                    notify::remember_skipped_notice(cx, &based_dir, &skipped);
                }
            }
            if let Some((based_dir, skipped)) = skipped_queries
                && let Some(note) = notify::skipped_queries_notice(cx, &based_dir, &skipped)
            {
                window.push_notification(note, cx);
                notify::remember_skipped_queries(cx, &based_dir, &skipped);
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

        let connection_tree = cx.new(|cx| ConnectionTree::new(registry.clone(), cx));

        let tab_manager = cx.new(|_| TabManager::with_tab(TabSpec::Home, home_arc.clone()));
        Workspaces::register(
            window.window_handle().window_id(),
            cx.entity().downgrade(),
            tab_manager.clone(),
            dock_area.clone(),
            registry.clone(),
            project,
            cx,
        );
        let command_palette = cx.new(|cx| CommandPalette::new(connection_tree.clone(), window, cx));
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
            window_id: window.window_handle().window_id(),
            session: tabs::SessionState::default(),
            pending_target_pick: None,
            session_edit_save: None,
            queued_dialog: None,
            tab_navigation: TabNavigationHistory::default(),
            close_prompt: None,
        };

        cx.subscribe(&tree_observe, |_ws, _, event, ecx| {
            let connection_tree::TreeEvent::OpenTab(spec) = event;
            inbox::open_tab(spec.clone(), ecx);
            ecx.notify();
        })
        .detach();

        cx.subscribe(&palette_observe, |ws, _, event, ecx| {
            match event {
                OpenTab(spec) => inbox::open_tab(spec.clone(), ecx),
                InjectSql { conn_id, sql } => {
                    inbox::inject_sql(conn_id.clone(), sql.clone(), ecx);
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
        cx.subscribe(
            &tab_mgr_observe,
            |ws, _, event: &tabs::TabEvent, ecx| match event {
                tabs::TabEvent::Edited => ws.note_editor_edit(ecx),
                tabs::TabEvent::Changed => ws.save_session(ecx),
            },
        )
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

    pub fn window_id(&self) -> WindowId {
        self.window_id
    }

    pub fn dock_area(&self) -> &Entity<DockArea> {
        &self.dock_area
    }

    pub fn tab_manager(&self) -> &Entity<TabManager> {
        &self.tab_manager
    }

    pub fn connection_tree(&self) -> &Entity<ConnectionTree> {
        &self.connection_tree
    }

    /// The assistant pane.
    pub fn chat(&self) -> &Entity<ChatPane> {
        &self.chat
    }

    pub fn command_palette(&self) -> &Entity<CommandPalette> {
        &self.command_palette
    }

    fn save_session(&mut self, cx: &mut Context<Self>) {
        let active_conn = self.focused_conn_id(cx);
        let tm = self.tab_manager.read(cx);
        let Some(snapshot) = tabs::SessionSnapshot::capture(tm, &self.session, active_conn) else {
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

    /// Write the session now, blocking, with every tab's current SQL and unsaved
    /// state. Call right before the window or app goes away: the async writer in
    /// [`Self::save_session`] would not get to run.
    pub fn flush_session(&mut self, cx: &mut Context<Self>) {
        self.session_edit_save = None;
        self.sync_tab_manager_from_dock(cx);
        let active_conn = self.focused_conn_id(cx);
        let Some(snapshot) =
            tabs::SessionSnapshot::capture(self.tab_manager.read(cx), &self.session, active_conn)
        else {
            return;
        };
        let store = storage::store(cx);
        if let Err(err) = Tokio::handle(cx).block_on(snapshot.save(&store)) {
            log::warn!("session flush failed: {err:#}");
        }
    }

    /// A query editor's text changed: save the session once typing pauses, so a
    /// crash or force-quit loses at most the last couple of seconds.
    pub fn note_editor_edit(&mut self, cx: &mut Context<Self>) {
        self.session_edit_save = Some(cx.spawn(async move |this, cx| {
            cx.background_executor().timer(SESSION_EDIT_DEBOUNCE).await;
            let _ = this.update(cx, |ws, cx| {
                ws.session_edit_save = None;
                ws.sync_tab_manager_from_dock(cx);
                ws.save_session(cx);
            });
        }));
    }

    fn restore_session(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let store = storage::store(cx);
        let handle = Tokio::handle(cx);
        let mut session = handle.block_on(tabs::SessionSnapshot::load(&store));
        {
            let registry = self.registry().read(cx);
            let engine_of = |id: &ConnectionId| registry.get(id, cx).map(|e| e.read(cx).engine());
            for tab in &mut session.tabs {
                if let Some(engine) = tab.spec.conn_id().and_then(engine_of) {
                    tab.spec.migrate_legacy_object(engine);
                }
            }
        }

        let active_spec = session
            .active
            .and_then(|idx| session.tabs.get(idx))
            .map(|t| t.spec.clone());
        self.session.set_pending(session.tabs);
        self.flush_pending_session_tabs(window, cx);
        if let Some(spec) = active_spec.filter(TabSpec::persist_in_session) {
            self.focus_existing_tab(&spec, window, cx);
        }
        self.sync_tab_manager_from_dock(cx);
        self.refresh_tab_strip_chrome(cx);
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

#[cfg(test)]
mod queued_dialog_tests {
    use std::path::PathBuf;

    use super::{QueuedDialog, merge_queued};

    #[test]
    fn quit_wins_and_newer_requests_replace_older() {
        let switch = |p: &str| QueuedDialog::SwitchProject(PathBuf::from(p));
        assert_eq!(
            merge_queued(None, QueuedDialog::CloseProject),
            QueuedDialog::CloseProject
        );
        assert_eq!(
            merge_queued(Some(QueuedDialog::CloseProject), switch("/b")),
            switch("/b")
        );
        assert_eq!(merge_queued(Some(switch("/a")), switch("/b")), switch("/b"));
        assert_eq!(
            merge_queued(Some(QueuedDialog::Quit), QueuedDialog::CloseProject),
            QueuedDialog::Quit
        );
        assert_eq!(
            merge_queued(Some(switch("/a")), QueuedDialog::Quit),
            QueuedDialog::Quit
        );
    }
}
