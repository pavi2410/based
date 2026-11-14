//! A live connection that stopped working: flip it to Failed so the rail and the
//! connection header offer Reconnect, instead of a green dot over failing tabs.

use based_core::{ConnectError, ConnectionErrorCategory};
use gpui_kit::App;

use super::Workspaces;
use crate::connection::{ConnectionId, ConnectionState};

/// Mark `conn` as lost after a query failed because the connection is gone
/// (see `based_postgres::is_connection_lost`). Does nothing unless it was connected.
pub fn mark_connection_lost(conn: &ConnectionId, detail: &str, cx: &mut App) {
    let Some(ws) = Workspaces::active(cx) else {
        return;
    };
    let Some(entry) = ws.read(cx).registry().read(cx).get(conn, cx).cloned() else {
        return;
    };
    entry.update(cx, |e, cx| {
        if matches!(e.state, ConnectionState::Connected(_)) {
            e.state = ConnectionState::Failed(ConnectError::new(
                ConnectionErrorCategory::NetUnreachable,
                format!("Connection lost: {detail}"),
            ));
            cx.notify();
        }
    });
}
