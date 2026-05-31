//! Deferred workspace operations — command palette routing and nav/tab queue draining.
//!
//! These are called from `render()` on every frame; see the comment in `render.rs` for why.

use gpui::{BorrowAppContext, Context};

use super::Workspace;
use super::tabs::{TabOpenQueue, TabSpec, WorkspaceNavQueue, enqueue_open_tab, enqueue_show_home};

impl Workspace {
    pub(crate) fn handle_palette_workspace_action(
        &mut self,
        action: crate::command_palette::WorkspacePaletteAction,
        cx: &mut Context<Self>,
    ) {
        use crate::command_palette::WorkspacePaletteAction;
        match action {
            WorkspacePaletteAction::NewLooseQuery => {
                super::query_lane::create_loose_query_from_palette(cx);
            }
            WorkspacePaletteAction::NewCollection => {
                super::query_lane::create_collection_from_palette(cx);
            }
            WorkspacePaletteAction::SelectNoEnvironment => {}
            WorkspacePaletteAction::OpenHome => enqueue_show_home(cx),
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

    pub(crate) fn drain_tab_open_queue(&mut self, cx: &mut Context<Self>) {
        if let Some(spec) = cx.update_global(|q: &mut TabOpenQueue, _| q.pending.take()) {
            self.pending_open_tab = Some(spec);
        }
    }

    pub(crate) fn flush_pending_open_tab(
        &mut self,
        window: &mut gpui::Window,
        cx: &mut Context<Self>,
    ) {
        self.drain_tab_open_queue(cx);
        if let Some(spec) = self.pending_open_tab.take() {
            self.dispatch_open_tab(spec, window, cx);
        }
    }

    pub(crate) fn flush_nav_queue(&mut self, window: &mut gpui::Window, cx: &mut Context<Self>) {
        let (show_home, open_wizard, toggle_side, toggle_left, open_notes, notes_version) = cx
            .update_global(|q: &mut WorkspaceNavQueue, _| {
                let home = q.show_home;
                let wizard = q.open_postgres_wizard;
                let side = q.toggle_side_pane.take();
                let left = q.toggle_left_pane.take();
                let notes = q.open_release_notes;
                let notes_version = q.pending_release_notes_version.take();
                q.show_home = false;
                q.open_postgres_wizard = false;
                q.open_release_notes = false;
                (home, wizard, side, left, notes, notes_version)
            });
        if let Some(pane) = toggle_side {
            self.toggle_side_pane(pane, cx);
        }
        if let Some(pane) = toggle_left {
            self.toggle_left_pane(pane, cx);
        }
        if show_home {
            self.show_home(window, cx);
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
}
