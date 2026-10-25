//! Entry point for panels to hand a question to the assistant.

use gpui_kit::App;

use crate::connection::ConnectionId;
use crate::workspace::WorkspaceRef;

/// Opens the assistant pane about `conn` and sends `text`.
pub fn ask_assistant(conn: ConnectionId, text: String, cx: &mut App) {
    let Some(ws) = cx.try_global::<WorkspaceRef>().map(|w| w.0.clone()) else {
        return;
    };
    ws.update(cx, |ws, cx| ws.ask_assistant(&conn, text, cx));
}
