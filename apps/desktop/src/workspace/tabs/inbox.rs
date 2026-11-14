//! Deferred requests for the workspace.
//!
//! Panels, menus and background tasks cannot always update [`Workspace`] directly
//! (it may already be mid-update), so they post a typed [`Request`] here and the
//! workspace applies it on its next render, in order.

use std::collections::VecDeque;

use gpui_kit::{App, WindowId};

use super::{TabSpec, Workspaces};
use crate::connection::ConnectionId;
use crate::workspace::chrome::{left_pane::LeftPane, side_pane::SidePane};

#[derive(Clone, Debug)]
pub enum Request {
    OpenTab(TabSpec),
    /// Put SQL in the focused editor for the connection, or open a tab holding it.
    InjectSql {
        conn_id: ConnectionId,
        sql: String,
    },
    /// SQL from an assistant card. `run`: open it in a new tab and run it read-only;
    /// otherwise put it in the editor, marked as AI-written.
    AiSql {
        conn_id: ConnectionId,
        sql: String,
        run: bool,
    },
    ShowHome,
    ToggleSidePane(SidePane),
    ToggleLeftPane(LeftPane),
    OpenReleaseNotes {
        version: String,
    },
    /// A connection just opened: add its first working tab beside the others.
    ConnectionOpened(ConnectionId),
}

#[derive(Default)]
pub struct Inbox {
    queue: VecDeque<Request>,
}

impl Inbox {
    pub fn push(&mut self, request: Request) {
        self.queue.push_back(request);
    }

    pub fn pop(&mut self) -> Option<Request> {
        self.queue.pop_front()
    }
}

/// Queue a request for the workspace the user is acting in. Pair with
/// [`request_workspace_flush`] unless a render is already coming.
pub fn post(request: Request, cx: &mut App) {
    if let Some(window) = Workspaces::active_window(cx) {
        Workspaces::post_to(window, request, cx);
    }
}

/// Queue a request for the workspace in `window` and wake it.
pub fn post_to(window: WindowId, request: Request, cx: &mut App) {
    Workspaces::post_to(window, request, cx);
    if let Some(ws) = Workspaces::for_window_id(window, cx) {
        ws.update(cx, |_, cx| cx.notify());
    }
}

/// Wake the active workspace so it drains its requests.
pub fn request_workspace_flush(cx: &mut App) {
    if let Some(ws) = Workspaces::active(cx) {
        ws.update(cx, |_, cx| cx.notify());
    }
}

pub fn open_tab(spec: TabSpec, cx: &mut App) {
    post(Request::OpenTab(spec), cx);
}

pub fn show_home(cx: &mut App) {
    post(Request::ShowHome, cx);
}

pub fn toggle_side_pane(pane: SidePane, cx: &mut App) {
    post(Request::ToggleSidePane(pane), cx);
}

pub fn open_release_notes(version: String, cx: &mut App) {
    post(Request::OpenReleaseNotes { version }, cx);
}

/// A connection in `window`'s workspace just opened.
pub fn connection_opened(window: WindowId, conn_id: ConnectionId, cx: &mut App) {
    post_to(window, Request::ConnectionOpened(conn_id), cx);
}

pub fn ai_sql(conn_id: ConnectionId, sql: String, run: bool, cx: &mut App) {
    post(Request::AiSql { conn_id, sql, run }, cx);
}

pub fn inject_sql(conn_id: ConnectionId, sql: String, cx: &mut App) {
    post(Request::InjectSql { conn_id, sql }, cx);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn requests_drain_in_order() {
        let mut inbox = Inbox::default();
        inbox.push(Request::ShowHome);
        inbox.push(Request::ToggleSidePane(SidePane::Chat));
        assert!(matches!(inbox.pop(), Some(Request::ShowHome)));
        assert!(matches!(inbox.pop(), Some(Request::ToggleSidePane(_))));
        assert!(inbox.pop().is_none());
    }
}
