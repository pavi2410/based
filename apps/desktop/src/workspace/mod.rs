// workspace/ — Workspace entity, DockArea, tabs, connection tree; shell chrome in `chrome/`.

pub mod chrome;
pub mod session;
pub mod tab_label;
pub mod tab_open;
pub mod tab_spec;
pub use tab_open::{
    SqlInject, TabManagerRef, TabOpenQueue, WorkspaceRef, enqueue_sql_inject, mark_query_tab_dirty,
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
pub mod welcome;

mod dock_utils;
mod inspector;
mod pop_out_impls;
mod tab_dispatch;

pub mod context;
pub mod query_lane;
pub mod templates;

use std::sync::Arc;

use dock_utils::{active_center_tab, center_tab_items, dock_area_present_views};
use inspector::render_inspector_body;

use std::path::PathBuf;

use gpui::{
    App, Context, Entity, EntityId, FocusHandle, Focusable, IntoElement, Render, SharedString,
    Window, prelude::*,
};
use gpui_component::{
    ActiveTheme, IndexPath,
    dock::{DockArea, DockEvent, DockItem, DockPlacement, PanelStyle, PanelView},
    select::{SelectEvent, SelectState},
    v_flex,
};

use crate::bindings::{
    CloseTab, CycleAppearance, DismissCommandPalette, OpenSettings, OpenWelcome,
    ToggleCommandPalette, ToggleHistoryPane, ToggleInspectorPane, ToggleSavedPane,
    ToggleSidebarRail,
};
use crate::command_palette::CommandPalette;
use crate::connection::ConnectionId;
use crate::connection::registry::ConnectionRegistry;
use crate::connection::{ConnectionEntry, ConnectionState};
use crate::project::{find_project_root, load_workspace_seed};
use crate::widgets::query_panel_extras::HistoryFilter;

use crate::storage;
use context::WorkspaceContext;
use templates::entries_from_workspace;

use chrome::{
    layout,
    left_pane::LeftPane,
    panes::{history_pane::render_history_pane, saved_pane::render_saved_pane},
    side_pane::{SidePane, render_side_pane},
    status_bar::{StatusBar, StatusBarModel},
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
    /// History pane filter chip (All / Saved / Today).
    history_filter: HistoryFilter,
    /// `(query_text, default_name)` when the user has clicked the star on a history row.
    pending_star: Option<(String, String)>,
    focus_handle: FocusHandle,
    project_title: SharedString,
    project_dir: Option<PathBuf>,
    session_restored: bool,
    pending_open_tab: Option<TabSpec>,
    /// Set by platform close; dialog is shown on the next [`Render`] (see `app::quit`).
    pub(crate) pending_close_confirm: bool,
    workspace_select: Entity<SelectState<Vec<SharedString>>>,
    env_select: Entity<SelectState<Vec<SharedString>>>,
    workspace_id: uuid::Uuid,
    pending_ctx_sync: Option<WorkspaceContext>,
}

impl Workspace {
    pub fn new(window: &mut Window, cx: &mut Context<Self>) -> Self {
        let project_dir = find_project_root();
        let legacy_entries = project_dir
            .as_ref()
            .map(|root| load_workspace_seed(root).1)
            .unwrap_or_default();

        let workspace_ctx = WorkspaceContext::load_initial(cx).unwrap_or_else(|e| {
            log::error!("workspace context load failed: {e:#}");
            WorkspaceContext {
                active: based_workspace::WorkspaceModel::new("Default"),
                summaries: vec![],
            }
        });
        cx.set_global(workspace_ctx.clone());

        let workspace_title: SharedString = workspace_ctx.active.name.clone().into();
        let workspace_id = workspace_ctx.active.id;

        let registry = cx.new(ConnectionRegistry::new);
        registry.update(cx, |reg, cx| {
            let mut seen = std::collections::HashSet::new();
            for entry in entries_from_workspace(&workspace_ctx.active) {
                seen.insert(entry.id.clone());
                reg.add(entry, cx);
            }
            for entry in legacy_entries {
                if seen.insert(entry.id.clone()) {
                    reg.add(entry, cx);
                }
            }
        });

        if registry.read(cx).connections().is_empty() {
            log::info!(
                "no connections loaded; add workspace templates or .based/config.toml entries"
            );
        }

        let dock_area = cx.new(|cx| {
            DockArea::new("workspace", Some(1), window, cx).panel_style(PanelStyle::TabBar)
        });

        let welcome = cx.new(|cx| WelcomePanel::new(window, cx));
        let welcome_panel = welcome.clone();
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

        let workspace_options: Vec<SharedString> = workspace_ctx
            .workspace_options()
            .into_iter()
            .map(SharedString::from)
            .collect();
        let workspace_select = cx.new(|cx| {
            SelectState::new(
                workspace_options,
                Some(IndexPath::new(workspace_ctx.active_workspace_index())),
                window,
                cx,
            )
        });
        let workspace_observe = workspace_select.clone();

        let env_options: Vec<SharedString> = workspace_ctx
            .environment_options()
            .into_iter()
            .map(SharedString::from)
            .collect();
        let env_select = cx.new(|cx| {
            SelectState::new(
                env_options,
                Some(IndexPath::new(workspace_ctx.active_environment_index())),
                window,
                cx,
            )
        });
        let env_observe = env_select.clone();

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
            pending_star: None,
            focus_handle: cx.focus_handle(),
            project_title: workspace_title,
            project_dir,
            session_restored: false,
            pending_open_tab: None,
            pending_close_confirm: false,
            workspace_select,
            env_select,
            workspace_id,
            pending_ctx_sync: None,
        };

        cx.subscribe(
            &workspace_observe,
            |ws, select, _: &SelectEvent<Vec<SharedString>>, cx| {
                let idx = select.read(cx).selected_index(cx).map(|p| p.row);
                ws.on_workspace_selected(idx, cx);
            },
        )
        .detach();

        cx.subscribe(
            &env_observe,
            |ws, select, _: &SelectEvent<Vec<SharedString>>, cx| {
                let idx = select.read(cx).selected_index(cx).map(|p| p.row);
                ws.on_environment_selected(idx, cx);
            },
        )
        .detach();

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
    }

    fn on_workspace_selected(&mut self, index: Option<usize>, cx: &mut Context<Self>) {
        let Some(idx) = index else {
            return;
        };
        let summaries = cx.global::<WorkspaceContext>().summaries.clone();
        let Some(summary) = summaries.get(idx) else {
            return;
        };
        if summary.id == self.workspace_id {
            return;
        }
        let store = storage::store(cx);
        let target = summary.id;
        let this = cx.entity().downgrade();
        cx.spawn(async move |_, cx| {
            let ctx = context::switch_workspace(store, target).await;
            cx.update(|cx| {
                let Some(this) = this.upgrade() else {
                    return;
                };
                match ctx {
                    Ok(ctx) => this.update(cx, |ws, cx| {
                        ws.apply_workspace_context(ctx, cx);
                    }),
                    Err(err) => log::warn!("workspace switch failed: {err:#}"),
                }
            })
        })
        .detach();
    }

    fn on_environment_selected(&mut self, index: Option<usize>, cx: &mut Context<Self>) {
        let Some(idx) = index else {
            return;
        };
        let ctx = cx.global::<WorkspaceContext>().clone();
        if idx == ctx.active_environment_index() {
            return;
        }
        let store = storage::store(cx);
        let this = cx.entity().downgrade();
        cx.spawn(async move |_, cx| {
            let updated = context::set_active_environment(store, &ctx, idx).await;
            cx.update(|cx| {
                let Some(this) = this.upgrade() else {
                    return;
                };
                match updated {
                    Ok(ctx) => this.update(cx, |ws, cx| {
                        ws.apply_workspace_context(ctx, cx);
                    }),
                    Err(err) => log::warn!("environment switch failed: {err:#}"),
                }
            })
        })
        .detach();
    }

    pub fn apply_workspace_context(&mut self, ctx: WorkspaceContext, cx: &mut Context<Self>) {
        self.workspace_id = ctx.active.id;
        self.project_title = ctx.active.name.clone().into();
        let ctx_for_registry = ctx.clone();
        cx.set_global(ctx.clone());
        self.pending_ctx_sync = Some(ctx);
        self.sync_registry_from_workspace(&ctx_for_registry, cx);
        cx.notify();
    }

    fn flush_pending_selector_sync(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let Some(ctx) = self.pending_ctx_sync.take() else {
            return;
        };
        let ws_opts: Vec<SharedString> = ctx
            .workspace_options()
            .into_iter()
            .map(SharedString::from)
            .collect();
        self.workspace_select.update(cx, |select, cx| {
            select.set_items(ws_opts, window, cx);
            select.set_selected_index(
                Some(IndexPath::new(ctx.active_workspace_index())),
                window,
                cx,
            );
        });
        let env_opts: Vec<SharedString> = ctx
            .environment_options()
            .into_iter()
            .map(SharedString::from)
            .collect();
        self.env_select.update(cx, |select, cx| {
            select.set_items(env_opts, window, cx);
            select.set_selected_index(
                Some(IndexPath::new(ctx.active_environment_index())),
                window,
                cx,
            );
        });
    }

    fn sync_registry_from_workspace(&mut self, ctx: &WorkspaceContext, cx: &mut Context<Self>) {
        self.registry.update(cx, |reg, cx| {
            let template_entries = entries_from_workspace(&ctx.active);
            for entry in template_entries {
                if reg.get(&entry.id, cx).is_none() {
                    reg.add(entry, cx);
                }
            }
        });
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
            WorkspacePaletteAction::SelectNoEnvironment => {
                self.on_environment_selected(Some(0), cx);
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
            // Switching panes drops any in-flight star prompt to avoid stale state.
            self.pending_star = None;
            Some(pane)
        };
        cx.notify();
    }

    pub fn set_history_filter(&mut self, filter: HistoryFilter, cx: &mut Context<Self>) {
        self.history_filter = filter;
        cx.notify();
    }

    pub fn set_pending_star(&mut self, query: String, name: String, cx: &mut Context<Self>) {
        self.pending_star = Some((query, name));
        cx.notify();
    }

    pub fn clear_pending_star(&mut self, cx: &mut Context<Self>) {
        self.pending_star = None;
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
        self.flush_pending_selector_sync(window, cx);
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

        let selected_connection = self.connection_tree.read(cx).selected_connection_entry(cx);
        let focused_conn_id = self.focused_conn_id(cx);
        let active_pane = self.active_side_pane;
        let history_filter = self.history_filter;
        let pending_star = self.pending_star.clone();
        let workspace_for_panes = this.clone();

        let side_pane: Option<gpui::AnyElement> = active_pane.map(|pane| {
            let body: gpui::AnyElement = match pane {
                SidePane::Inspector => {
                    render_inspector_body(selected_connection, window, cx).into_any_element()
                }
                SidePane::History => render_history_pane(
                    workspace_for_panes.clone(),
                    focused_conn_id.clone(),
                    history_filter,
                    pending_star.clone(),
                    cx,
                )
                .into_any_element(),
                SidePane::Saved => {
                    render_saved_pane(focused_conn_id.clone(), cx).into_any_element()
                }
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
            .child(Topbar::new(
                self.project_title.clone(),
                this.clone(),
                self.workspace_select.clone(),
                self.env_select.clone(),
            ))
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
                },
                this.clone(),
                active_pane,
                self.active_left_pane,
            ))
            .child(self.command_palette.clone());

        chrome::overlays::stack_gpui_overlays(main, window, cx)
    }
}
