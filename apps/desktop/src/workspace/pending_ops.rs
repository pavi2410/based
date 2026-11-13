//! Deferred workspace operations — command palette routing and nav/tab queue draining.
//!
//! These are called from `render()` on every frame; see the comment in `render.rs` for why.

use gpui_kit::{BorrowAppContext, Context};

use super::Workspace;
use super::panels::query_editor::QueryEditorPanel;
use super::tabs::{
    CloseScope, Inbox, OpenOutcome, QueryEditorInit, Request, TabId, TabSpec, inbox,
};
use crate::app::logging::open_logs;
use crate::app::updater::check_now;
use crate::command_palette::WorkspacePaletteAction;
use crate::connection::{ConnectionId, EngineKind};
use crate::project::prompt_open_project_in_new_window;
use crate::project::prompt_open_project_in_window;
use crate::project::request_close_project_in_window;

impl Workspace {
    pub(crate) fn handle_palette_workspace_action(
        &mut self,
        action: WorkspacePaletteAction,
        cx: &mut Context<Self>,
    ) {
        match action {
            WorkspacePaletteAction::NewLooseQuery => {
                super::query_lane::create_loose_query_from_palette(cx);
            }
            WorkspacePaletteAction::NewCollection => {
                super::query_lane::create_collection_from_palette(cx);
            }
            WorkspacePaletteAction::SelectNoEnvironment => {}
            WorkspacePaletteAction::OpenHome => inbox::show_home(cx),
            WorkspacePaletteAction::OpenLogs => open_logs(),
            WorkspacePaletteAction::CheckForUpdates => check_now(cx),
            WorkspacePaletteAction::OpenProject => {
                prompt_open_project_in_window(cx);
            }
            WorkspacePaletteAction::OpenProjectInNewWindow => {
                prompt_open_project_in_new_window(cx);
            }
            WorkspacePaletteAction::CloseProject => {
                request_close_project_in_window(cx);
            }
        }
    }

    /// Apply every queued [`Request`], oldest first.
    pub(crate) fn drain_inbox(&mut self, window: &mut gpui_kit::Window, cx: &mut Context<Self>) {
        while let Some(request) = cx.update_global(|inbox: &mut Inbox, _| inbox.pop()) {
            match request {
                Request::OpenTab(spec) => {
                    self.dispatch_open_tab(spec, window, cx);
                }
                Request::InjectSql { conn_id, sql } => self.inject_sql(conn_id, sql, window, cx),
                Request::ShowHome => self.show_home(window, cx),
                Request::ToggleSidePane(pane) => self.toggle_side_pane(pane, cx),
                Request::ToggleLeftPane(pane) => self.toggle_left_pane(pane, cx),
                Request::OpenReleaseNotes { version } => {
                    self.dispatch_open_tab(TabSpec::ReleaseNotes { version }, window, cx);
                }
                Request::ConnectionOpened(conn_id) => {
                    self.open_first_tab_for_connection(conn_id, window, cx);
                }
            }
        }
        self.flush_pending_session_tabs(window, cx);
    }

    /// After a connection opens, add one tab to work in: a query editor (SQL) or
    /// the console (Redis). Mongo collections are already listed in the catalog.
    /// Existing tabs are never touched; an untouched lone Home tab gives way.
    pub(crate) fn open_first_tab_for_connection(
        &mut self,
        conn_id: ConnectionId,
        window: &mut gpui_kit::Window,
        cx: &mut Context<Self>,
    ) {
        let Some(engine) = self
            .registry
            .read(cx)
            .get(&conn_id, cx)
            .map(|e| e.read(cx).engine())
        else {
            return;
        };
        if engine == EngineKind::MongoDB {
            return;
        }
        let home_id = self.home_panel.entity_id();
        let only_home = {
            let tm = self.tab_manager.read(cx);
            tm.tabs.len() == 1 && tm.tabs[0].view().entity_id() == home_id
        };
        let outcome = self.dispatch_open_tab(TabSpec::blank_query_editor(conn_id), window, cx);
        if only_home && matches!(outcome, OpenOutcome::Opened(_)) {
            self.close_tabs(CloseScope::Tab(home_id), window, cx);
        }
    }

    /// Replace the SQL in the active editor for `conn_id`, or open a tab holding it.
    fn inject_sql(
        &mut self,
        conn_id: ConnectionId,
        sql: String,
        window: &mut gpui_kit::Window,
        cx: &mut Context<Self>,
    ) {
        let editor = self
            .tab_manager
            .read(cx)
            .active_tab()
            .and_then(|t| t.downcast::<QueryEditorPanel>())
            .filter(|e| e.read(cx).connection_id() == &conn_id);
        match editor {
            Some(editor) => editor.update(cx, |e, cx| e.set_sql(&sql, window, cx)),
            None => {
                let init = QueryEditorInit {
                    sql: Some(sql),
                    auto_run: false,
                    saved: None,
                    dirty: false,
                };
                self.dispatch_open_tab(TabSpec::QueryEditor { conn_id, init }, window, cx);
            }
        }
    }

    pub(crate) fn flush_pending_session_tabs(
        &mut self,
        window: &mut gpui_kit::Window,
        cx: &mut Context<Self>,
    ) {
        let mut unsupported = Vec::new();
        // In saved order, so the strip comes back as it was.
        for (index, saved) in self.session.to_open() {
            // A tab that is already open stands for this entry, unless another
            // entry claimed it. Query editors never match by content: two
            // equal blank editors are two tabs.
            let already_open = (!matches!(saved.spec, TabSpec::QueryEditor { .. }))
                .then(|| {
                    self.tab_manager
                        .read(cx)
                        .tabs
                        .iter()
                        .find(|t| t.spec == saved.spec && !self.session.is_linked(t.id))
                        .map(|t| t.id)
                })
                .flatten();
            let outcome = match already_open {
                Some(id) => OpenOutcome::Focused(id),
                None => self.dispatch_open_tab(saved.spec.clone(), window, cx),
            };
            match outcome {
                OpenOutcome::Opened(id) | OpenOutcome::Focused(id) => {
                    self.session.link(index, id);
                    if saved.pinned {
                        self.tab_manager
                            .update(cx, |tm, ecx| tm.set_pinned(id, true, ecx));
                    }
                }
                OpenOutcome::NotYet => {}
                OpenOutcome::Unsupported => {
                    log::info!("dropping saved tab that cannot open: {:?}", saved.spec);
                    unsupported.push(index);
                }
            }
        }
        // Last first, so removing one doesn't shift the others.
        for index in unsupported.into_iter().rev() {
            self.session.drop_pending(index);
        }
    }

    pub(crate) fn forget_session_tab(&mut self, id: TabId) {
        self.session.forget(id);
    }

    pub(crate) fn should_preserve_restored_tabs(&self, conn_id: &ConnectionId) -> bool {
        self.session.awaits(conn_id)
    }
}
