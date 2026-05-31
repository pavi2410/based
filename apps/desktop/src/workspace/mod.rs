// workspace/ — Workspace entity, DockArea, tabs, connection tree; shell chrome in `chrome/`.

pub mod chrome;
pub mod session;
pub mod tab_label;
pub mod tab_open;
pub mod tab_spec;
pub use tab_open::{
    DockAreaRef, SqlInject, TabManagerRef, TabOpenQueue, WorkspaceNavQueue, WorkspaceRef,
    enqueue_open_tab, enqueue_show_welcome, enqueue_sql_inject, mark_query_tab_dirty,
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
pub mod release_notes;
pub mod welcome;

mod dock_utils;
mod inspector;
mod pop_out_impls;
pub mod project_query;
mod tab_commands;
mod tab_dispatch;
mod tab_infer;
mod tab_navigation;

pub mod context;
pub mod query_lane;
pub mod templates;

use std::sync::Arc;

use dock_utils::{active_center_tab, center_tab_items, wrap_center_root};
use inspector::render_inspector_body;

use std::path::PathBuf;

use gpui::{
    App, Context, Entity, EntityId, FocusHandle, Focusable, IntoElement, Render, SharedString,
    Window, div, prelude::*,
};
use gpui_component::{
    ActiveTheme, Placement,
    dock::{DockArea, DockEvent, DockItem, DockPlacement, PanelStyle, PanelView},
    v_flex,
};

use crate::bindings::{
    CloseAllTabs, CloseCleanTabs, CloseOtherTabs, CloseTab, CloseTabsLeft, CloseTabsRight,
    CycleAppearance, DismissCommandPalette, GoBackTab, GoForwardTab, NewQuery, OpenSettings,
    OpenWelcome, PinTab, SplitPaneBottom, SplitPaneLeft, SplitPaneRight, SplitPaneTop,
    ToggleCommandPalette, ToggleHistoryPane, ToggleInspectorPane, ToggleSavedPane,
    ToggleSidebarRail,
};
use crate::command_palette::CommandPalette;
use crate::connection::ConnectionId;
use crate::connection::registry::ConnectionRegistry;
use crate::connection::{ConnectionEntry, ConnectionState};
use crate::query_store::QueryStore;
use based_project::ProjectQuery;

use crate::project::{ProjectContext, find_project_root, loader::entry_from_project};

use project_query::{OpenQueryResult, open_project_query, tab_spec_for_query};

use crate::storage;
use crate::widgets::query_panel_extras::HistoryFilter;
use context::WorkspaceContext;

use tab_navigation::TabNavigationHistory;

use chrome::{
    layout,
    left_pane::LeftPane,
    panes::{history_pane::render_history_pane, saved_pane::render_saved_pane},
    side_pane::{SidePane, render_side_pane},
    status_bar::{StatusBar, StatusBarModel},
    target_picker::render_target_picker,
    topbar::Topbar,
};
use welcome::WelcomePanel;

pub struct Workspace {
    registry: Entity<ConnectionRegistry>,
    welcome_panel: Entity<WelcomePanel>,
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

        let welcome = cx.new(|cx| WelcomePanel::new(window, cx));
        let welcome_panel = welcome.clone();
        let weak_dock = dock_area.downgrade();
        let tabs = DockItem::tab(welcome, &weak_dock, window, cx);
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
            welcome_panel,
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
        let tm = self.tab_manager.read(cx);
        if tm.tabs.is_empty() {
            return;
        }
        let snapshot = session::SessionSnapshot {
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
        let session = handle.block_on(session::SessionSnapshot::load(&store));

        if let Some(conn_key) = session.active_connection_id.clone() {
            let conn_id = ConnectionId(conn_key);
            self.connection_tree.update(cx, |tree, ecx| {
                tree.focus_connection_by_id(&conn_id, ecx);
            });
        }

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

    pub fn set_pending_target_pick(&mut self, query: ProjectQuery, candidates: Vec<ConnectionId>) {
        self.pending_target_pick = Some((query, candidates));
    }

    pub fn resolve_pending_target(&mut self, conn_id: ConnectionId, cx: &mut Context<Self>) {
        if let Some((query, _)) = self.pending_target_pick.take() {
            self.pending_open_tab = Some(tab_spec_for_query(&query, conn_id));
            cx.notify();
        }
    }

    pub fn cancel_pending_target_pick(&mut self, cx: &mut Context<Self>) {
        if self.pending_target_pick.take().is_some() {
            cx.notify();
        }
    }

    fn open_project_query_by_path(&mut self, path: &str, cx: &mut Context<Self>) {
        let store = cx.global::<QueryStore>();
        let Some(query) = store.project_queries().iter().find(|q| q.path == path) else {
            log::warn!("project query not found: {path}");
            return;
        };
        let focused = self.focused_conn_id(cx);
        match open_project_query(query, self.registry.read(cx), cx, focused.as_ref()) {
            OpenQueryResult::Open(spec) => {
                self.pending_open_tab = Some(spec);
            }
            OpenQueryResult::PickConnection { candidates, .. } => {
                self.pending_target_pick = Some((query.clone(), candidates));
            }
            OpenQueryResult::Error(msg) => log::warn!("{msg}"),
        }
    }

    pub fn sync_project_context(&mut self, cx: &mut Context<Self>) {
        if let Some(pctx) = cx.try_global::<ProjectContext>() {
            self.project_title = pctx.project_name().into();
            cx.notify();
        }
    }

    pub fn has_dirty_tabs(&self, cx: &App) -> bool {
        self.tab_manager.read(cx).tabs.iter().any(|t| t.dirty)
    }

    pub fn apply_opened_project(&mut self, root: PathBuf, cx: &mut Context<Self>) {
        self.project_dir = Some(root);
        if let Some(pctx) = cx.try_global::<ProjectContext>() {
            self.project_title = pctx.project_name().into();
        }
        self.connection_tree.update(cx, |_, cx| cx.notify());
        cx.notify();
    }

    pub fn apply_workspace_context(&mut self, ctx: WorkspaceContext, cx: &mut Context<Self>) {
        if let Some(pctx) = cx.try_global::<ProjectContext>() {
            self.project_title = pctx.project_name().into();
        } else {
            self.project_title = ctx.active.name.clone().into();
        }
        cx.set_global(ctx.clone());
        cx.notify();
    }

    pub fn persist_postgres_template(
        &mut self,
        config: &crate::postgres::PostgresConfig,
        cx: &mut Context<Self>,
    ) {
        let ctx = cx.global::<WorkspaceContext>().clone();
        let existing = ctx
            .active
            .connection_templates
            .iter()
            .find(|t| t.label == config.label)
            .map(|t| t.id);
        let template = templates::template_from_postgres_config(config, existing);
        let store = storage::store(cx);
        let workspace_id = ctx.active.id;
        let this = cx.entity().downgrade();
        cx.spawn(async move |_, cx| {
            if let Err(err) = store
                .upsert_connection_template(workspace_id, &template)
                .await
            {
                log::warn!("persist connection template failed: {err:#}");
                return;
            }
            let refreshed = context::refresh_context(store, workspace_id).await;
            cx.update(|cx| {
                let Some(this) = this.upgrade() else {
                    return;
                };
                if let Ok(ctx) = refreshed {
                    let entry = templates::resolve_template_entry(&ctx.active, &template).ok();
                    this.update(cx, |ws, cx| {
                        ws.apply_workspace_context(ctx, cx);
                        if let Some(entry) = entry {
                            ws.registry.update(cx, |reg, cx| {
                                if reg.get(&entry.id, cx).is_none() {
                                    reg.add(entry, cx);
                                }
                            });
                        }
                    });
                }
            })
        })
        .detach();
    }

    fn handle_palette_workspace_action(
        &mut self,
        action: crate::command_palette::WorkspacePaletteAction,
        cx: &mut Context<Self>,
    ) {
        use crate::command_palette::WorkspacePaletteAction;
        match action {
            WorkspacePaletteAction::NewLooseQuery => {
                query_lane::create_loose_query_from_palette(cx);
            }
            WorkspacePaletteAction::NewCollection => {
                query_lane::create_collection_from_palette(cx);
            }
            WorkspacePaletteAction::SelectNoEnvironment => {}
            WorkspacePaletteAction::OpenWelcome => enqueue_show_welcome(cx),
            WorkspacePaletteAction::OpenOnboarding => crate::app::shell::open_onboarding(cx),
            WorkspacePaletteAction::CheckForUpdates => crate::app::updater::check_now(cx),
            WorkspacePaletteAction::OpenProject => {
                crate::project::prompt_open_project_in_window(cx);
            }
            WorkspacePaletteAction::OpenProjectInNewWindow => {
                crate::project::prompt_open_project_in_new_window(cx);
            }
        }
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

    /// Open the Postgres connection wizard in a new center tab.
    pub fn open_postgres_wizard_tab(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let panel = cx.new(|cx| crate::postgres::wizard::ConnectionWizardPanel::new(window, cx));
        let arc: Arc<dyn PanelView> = Arc::new(panel);
        self.dock_area.update(cx, |dock, ecx| {
            dock.add_panel(arc, DockPlacement::Center, None, window, ecx);
        });
        cx.notify();
    }

    /// Focus the Welcome tab, re-adding it to the center strip if it was replaced.
    pub fn show_welcome(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let welcome: Arc<dyn PanelView> = Arc::new(self.welcome_panel.clone());
        let dock = self.dock_area.read(cx);
        let welcome_ix = center_tab_items(dock.center()).and_then(|items| {
            items
                .iter()
                .position(|p| p.panel_name(cx) == "WelcomePanel")
        });
        let is_active = active_center_tab(dock.center(), cx)
            .is_some_and(|(p, _)| p.panel_name(cx) == "WelcomePanel");

        self.dock_area.update(cx, |dock, ecx| match welcome_ix {
            Some(_ix) if is_active => {}
            Some(ix) => {
                if let Some(panel) =
                    center_tab_items(dock.center()).and_then(|items| items.get(ix).cloned())
                {
                    dock.remove_panel(panel, DockPlacement::Center, window, ecx);
                    dock.add_panel(welcome.clone(), DockPlacement::Center, None, window, ecx);
                }
            }
            None => {
                dock.add_panel(welcome, DockPlacement::Center, None, window, ecx);
            }
        });

        self.welcome_panel
            .read(cx)
            .focus_handle(cx)
            .focus(window, cx);
        cx.notify();
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

    fn flush_nav_queue(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let (show_welcome, open_wizard, toggle_side, toggle_left, open_notes, notes_version) = cx
            .update_global(|q: &mut WorkspaceNavQueue, _| {
                let welcome = q.show_welcome;
                let wizard = q.open_postgres_wizard;
                let side = q.toggle_side_pane.take();
                let left = q.toggle_left_pane.take();
                let notes = q.open_release_notes;
                let notes_version = q.pending_release_notes_version.take();
                q.show_welcome = false;
                q.open_postgres_wizard = false;
                q.open_release_notes = false;
                (welcome, wizard, side, left, notes, notes_version)
            });
        if let Some(pane) = toggle_side {
            self.toggle_side_pane(pane, cx);
        }
        if let Some(pane) = toggle_left {
            self.toggle_left_pane(pane, cx);
        }
        if show_welcome {
            self.show_welcome(window, cx);
        }
        if open_wizard {
            self.open_postgres_wizard_tab(window, cx);
        }
        if open_notes && let Some(version) = notes_version {
            enqueue_open_tab(TabSpec::ReleaseNotes { version }, cx);
            self.drain_tab_open_queue(cx);
            self.flush_pending_open_tab(window, cx);
        }
    }

    pub(crate) fn sync_tab_manager_from_dock(&mut self, cx: &mut Context<Self>) {
        let dock = self.dock_area.read(cx);
        let entries: Vec<_> = center_tab_items(dock.center())
            .into_iter()
            .flatten()
            .map(|panel| {
                let view = panel.view();
                let spec = tab_infer::infer_tab_spec(panel, cx);
                (view, spec)
            })
            .collect();
        let active = active_center_tab(dock.center(), cx).map(|(panel, _)| panel.view());
        self.tab_manager.update(cx, |tm, ecx| {
            tm.reconcile_dock_tabs(&entries, active, ecx);
        });
        self.record_tab_navigation(cx);
        self.refresh_tab_strip_chrome(cx);
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
        pop_out::panel_type_allows_tab_close(panel.panel_name(cx))
            && !self.is_tab_pinned(panel_id, cx)
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

    pub fn active_center_panel_id(&self, cx: &App) -> Option<EntityId> {
        let dock = self.dock_area.read(cx);
        active_center_tab(dock.center(), cx).map(|(panel, _)| panel.panel_id(cx))
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
        crate::project::open::maybe_show_pending_project_switch_dialog(self, window, cx);
        if crate::project::drain_pending_reload(cx)
            && let Some(pctx) = cx.try_global::<ProjectContext>()
        {
            self.project_title = pctx.project_name().into();
        }
        if !self.session_restored {
            self.session_restored = true;
            self.restore_session(window, cx);
        }
        self.flush_nav_queue(window, cx);
        self.flush_pending_open_tab(window, cx);
        let this = cx.entity().clone();
        let conn_list: Vec<Entity<ConnectionEntry>> = self.registry.read(cx).connections().to_vec();
        let conn_count = conn_list.len();
        let connected_count = conn_list
            .iter()
            .filter(|ent| matches!(ent.read(cx).state, ConnectionState::Connected(_)))
            .count();

        let selected_connection = self.connection_tree.read(cx).selected_connection_entry(cx);
        let focused_conn_id = self.focused_conn_id(cx);
        let active_pane = self.active_side_pane;
        let history_filter = self.history_filter;
        let workspace_for_panes = this.clone();
        let target_pick = self.pending_target_pick.clone();

        let side_pane: Option<gpui::AnyElement> = active_pane.map(|pane| {
            let body: gpui::AnyElement = match pane {
                SidePane::Inspector => {
                    render_inspector_body(selected_connection, window, cx).into_any_element()
                }
                SidePane::History => render_history_pane(
                    workspace_for_panes.clone(),
                    focused_conn_id.clone(),
                    history_filter,
                    cx,
                )
                .into_any_element(),
                SidePane::Saved => render_saved_pane(
                    focused_conn_id.clone(),
                    self.registry.clone(),
                    this.clone(),
                    cx,
                )
                .into_any_element(),
            };
            render_side_pane(pane, body, cx).into_any_element()
        });

        let sidebar = match self.active_left_pane {
            LeftPane::Browser => self.connection_tree.clone().into_any_element(),
            LeftPane::Workspace => v_flex()
                .size_full()
                .min_h_0()
                .overflow_hidden()
                .child(crate::workspace::query_lane::render_query_lane(cx))
                .into_any_element(),
        };

        let body = layout::render_body_row(
            self.sidebar_collapsed,
            sidebar,
            self.dock_area.clone(),
            side_pane,
            cx,
        );

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
            .on_action(
                window.listener_for(&this, |ws, _: &OpenWelcome, window, cx| {
                    ws.show_welcome(window, cx);
                }),
            )
            .on_action(window.listener_for(&this, |ws, _: &CloseTab, window, cx| {
                ws.close_active_center_tab(window, cx);
            }))
            .on_action(window.listener_for(&this, |ws, _: &GoBackTab, window, cx| {
                ws.go_back_tab(window, cx);
            }))
            .on_action(
                window.listener_for(&this, |ws, _: &GoForwardTab, window, cx| {
                    ws.go_forward_tab(window, cx);
                }),
            )
            .on_action(window.listener_for(&this, |ws, _: &NewQuery, window, cx| {
                ws.open_new_query_tab(window, cx);
            }))
            .on_action(
                window.listener_for(&this, |ws, _: &CloseAllTabs, window, cx| {
                    ws.close_all_tabs(window, cx);
                }),
            )
            .on_action(
                window.listener_for(&this, |ws, _: &CloseCleanTabs, window, cx| {
                    ws.close_clean_tabs(window, cx);
                }),
            )
            .on_action(
                window.listener_for(&this, |ws, _: &CloseOtherTabs, window, cx| {
                    if let Some(id) = ws.active_center_panel_id(cx) {
                        ws.close_other_tabs(id, window, cx);
                    }
                }),
            )
            .on_action(
                window.listener_for(&this, |ws, _: &CloseTabsLeft, window, cx| {
                    if let Some(id) = ws.active_center_panel_id(cx) {
                        ws.close_tabs_to_left(id, window, cx);
                    }
                }),
            )
            .on_action(
                window.listener_for(&this, |ws, _: &CloseTabsRight, window, cx| {
                    if let Some(id) = ws.active_center_panel_id(cx) {
                        ws.close_tabs_to_right(id, window, cx);
                    }
                }),
            )
            .on_action(window.listener_for(&this, |ws, _: &PinTab, _, cx| {
                if let Some(id) = ws.active_center_panel_id(cx) {
                    ws.toggle_pin_tab(id, cx);
                }
            }))
            .on_action(
                window.listener_for(&this, |ws, _: &SplitPaneTop, window, cx| {
                    ws.split_center_pane(Placement::Top, window, cx);
                }),
            )
            .on_action(
                window.listener_for(&this, |ws, _: &SplitPaneBottom, window, cx| {
                    ws.split_center_pane(Placement::Bottom, window, cx);
                }),
            )
            .on_action(
                window.listener_for(&this, |ws, _: &SplitPaneLeft, window, cx| {
                    ws.split_center_pane(Placement::Left, window, cx);
                }),
            )
            .on_action(
                window.listener_for(&this, |ws, _: &SplitPaneRight, window, cx| {
                    ws.split_center_pane(Placement::Right, window, cx);
                }),
            )
            .on_action(
                window.listener_for(&this, |ws, _: &ToggleInspectorPane, _, cx| {
                    ws.toggle_side_pane(SidePane::Inspector, cx);
                }),
            )
            .on_action(
                window.listener_for(&this, |ws, _: &ToggleHistoryPane, _, cx| {
                    ws.toggle_side_pane(SidePane::History, cx);
                }),
            )
            .on_action(
                window.listener_for(&this, |ws, _: &ToggleSavedPane, _, cx| {
                    ws.toggle_side_pane(SidePane::Saved, cx);
                }),
            )
            .bg(cx.theme().background)
            .child(Topbar::new(self.registry.clone()))
            .child(body)
            .child(StatusBar::new(
                StatusBarModel {
                    connection_count: conn_count,
                    connected_count,
                    scope_label: self.project_title.clone(),
                    history_ready: !cx
                        .global::<crate::query_store::QueryStore>()
                        .history
                        .recent(1)
                        .is_empty(),
                    update: crate::app::updater::coordinator_snapshot(cx),
                },
                active_pane,
                self.active_left_pane,
                self.registry.clone(),
            ))
            .child(self.command_palette.clone());

        let stacked = chrome::overlays::stack_gpui_overlays(main, window, cx);
        div()
            .size_full()
            .relative()
            .child(stacked)
            .when_some(target_pick, |layer, pick| {
                layer.child(render_target_picker(
                    &pick.0,
                    &pick.1,
                    &self.registry,
                    this.clone(),
                    cx,
                ))
            })
    }
}
