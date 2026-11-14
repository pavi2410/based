use gpui_kit::{Context, Window};

use crate::connection::AnyConnection;
use crate::workspace::Workspaces;
use crate::workspace::tabs::inbox;

use super::ConnectionTree;

impl ConnectionTree {
    /// A connection just opened: show its catalog and ask the workspace for a first
    /// tab to work in. Open tabs are never replaced; restored tabs waiting on this
    /// connection come back on their own instead.
    pub(crate) fn open_connected_workspace(
        &mut self,
        idx: usize,
        ac: &AnyConnection,
        _window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let Some(conn_ent) = self.registry.read(cx).connections().get(idx).cloned() else {
            return;
        };
        let conn_id = conn_ent.read(cx).id.clone();

        self.selected_connection = Some(idx);
        self.selected_object = None;
        self.set_connection_expanded(idx, true, cx);
        self.load_objects_for_connection(idx, ac.clone(), cx);

        // This tree's own window: connects finish in the background, maybe while
        // another window has focus.
        let Some(window) = Workspaces::window_with_registry(&self.registry, cx) else {
            return;
        };
        let Some(ws) = Workspaces::for_window_id(window, cx) else {
            return;
        };
        if ws.read(cx).should_preserve_restored_tabs(&conn_id) {
            ws.update(cx, |_, cx| cx.notify());
            return;
        }
        inbox::connection_opened(window, conn_id, cx);
    }
}
