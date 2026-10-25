//! Deferred workspace operations — command palette routing and nav/tab queue draining.
//!
//! These are called from `render()` on every frame; see the comment in `render.rs` for why.

use gpui_kit::{BorrowAppContext, Context};

use super::Workspace;
use super::panels::query_editor::QueryEditorPanel;
use super::tabs::{Inbox, QueryEditorInit, Request, TabSpec, inbox};
use crate::app::logging::open_logs;
use crate::app::updater::check_now;
use crate::command_palette::WorkspacePaletteAction;
use crate::connection::ConnectionId;
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
            }
        }
        self.flush_pending_session_tabs(window, cx);
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
                let init = QueryEditorInit::Sql {
                    sql: Some(sql),
                    auto_run: false,
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
        for spec in self.session.to_open() {
            let open = self
                .tab_manager
                .read(cx)
                .tabs
                .iter()
                .any(|t| t.spec == spec);
            if open || self.dispatch_open_tab(spec.clone(), window, cx) {
                self.session.mark_opened(spec);
            }
        }
    }

    pub(crate) fn forget_session_tab(&mut self, spec: &TabSpec) {
        self.session.forget(spec);
    }

    pub(crate) fn should_preserve_restored_tabs(&self, conn_id: &ConnectionId) -> bool {
        self.session.awaits(conn_id)
    }
}
