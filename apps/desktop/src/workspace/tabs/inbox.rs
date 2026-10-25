//! Deferred requests for the workspace.
//!
//! Panels, menus and background tasks cannot always update [`Workspace`] directly
//! (it may already be mid-update), so they post a typed [`Request`] here and the
//! workspace applies it on its next render, in order.

use std::collections::VecDeque;

use gpui_kit::{App, BorrowAppContext, Global};

use super::{TabSpec, WorkspaceRef};
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
    ShowHome,
    ToggleSidePane(SidePane),
    ToggleLeftPane(LeftPane),
    OpenReleaseNotes {
        version: String,
    },
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

impl Global for Inbox {}

/// Queue a request. Pair with [`request_workspace_flush`] unless a render is already coming.
pub fn post(request: Request, cx: &mut impl BorrowAppContext) {
    cx.update_global(|inbox: &mut Inbox, _| inbox.push(request));
}

/// Wake the workspace so it drains the inbox.
pub fn request_workspace_flush(cx: &mut App) {
    if let Some(ws) = cx.try_global::<WorkspaceRef>().map(|w| w.0.clone()) {
        ws.update(cx, |_, cx| cx.notify());
    }
}

pub fn open_tab(spec: TabSpec, cx: &mut impl BorrowAppContext) {
    post(Request::OpenTab(spec), cx);
}

pub fn show_home(cx: &mut impl BorrowAppContext) {
    post(Request::ShowHome, cx);
}

pub fn toggle_side_pane(pane: SidePane, cx: &mut impl BorrowAppContext) {
    post(Request::ToggleSidePane(pane), cx);
}

pub fn open_release_notes(version: String, cx: &mut impl BorrowAppContext) {
    post(Request::OpenReleaseNotes { version }, cx);
}

pub fn inject_sql(conn_id: ConnectionId, sql: String, cx: &mut impl BorrowAppContext) {
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
