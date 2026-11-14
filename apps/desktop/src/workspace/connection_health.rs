//! A live connection that stopped working: flip it to Failed so the rail and the
//! connection header offer Reconnect, instead of a green dot over failing tabs.

use based_core::{ConnectError, ConnectionErrorCategory};
use gpui_kit::{App, EntityId};

use super::Workspaces;
use crate::connection::{ConnectionId, ConnectionState};

/// Mark `conn` as lost after a query in panel `panel_id` failed because the
/// connection is gone (see `based_postgres::is_connection_lost`). Does nothing
/// unless it was connected.
pub fn mark_connection_lost(panel_id: EntityId, conn: &ConnectionId, detail: &str, cx: &mut App) {
    let Some(registry) = Workspaces::registry_for_panel(panel_id, cx) else {
        return;
    };
    let Some(entry) = registry.read(cx).get(conn, cx).cloned() else {
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
