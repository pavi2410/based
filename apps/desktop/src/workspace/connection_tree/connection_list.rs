use gpui::{App, SharedString};
use gpui_component::ActiveTheme;

use crate::connection::{ConnectionState, EngineKind};

use super::ConnectionTree;

#[derive(Clone)]
pub(crate) struct ConnectionRow {
    pub(crate) idx: usize,
    pub(crate) conn_label: SharedString,
    pub(crate) engine: EngineKind,
    pub(crate) state_color: gpui::Hsla,
    pub(crate) is_connected: bool,
    pub(crate) is_connecting: bool,
    pub(crate) is_failed: bool,
    pub(crate) fail_reason: Option<String>,
}

fn connection_state_dot(state: &ConnectionState, t: &gpui_component::Theme) -> gpui::Hsla {
    match state {
        ConnectionState::Disconnected => t.muted_foreground.opacity(0.75),
        ConnectionState::Connecting { .. } => t.warning_foreground,
        ConnectionState::Connected(_) => t.green_light,
        ConnectionState::Failed { .. } => t.danger_foreground,
    }
}

pub(crate) fn build_connection_rows(tree: &ConnectionTree, cx: &App) -> Vec<ConnectionRow> {
    tree.registry
        .read(cx)
        .connections()
        .iter()
        .enumerate()
        .map(|(idx, ent)| {
            let entry = ent.read(cx);
            ConnectionRow {
                idx,
                conn_label: entry.config.label().to_string().into(),
                engine: entry.config.engine(),
                state_color: connection_state_dot(&entry.state, cx.theme()),
                is_connected: matches!(entry.state, ConnectionState::Connected(_)),
                is_connecting: matches!(entry.state, ConnectionState::Connecting { .. }),
                is_failed: matches!(entry.state, ConnectionState::Failed { .. }),
                fail_reason: match &entry.state {
                    ConnectionState::Failed { reason, .. } => Some(reason.clone()),
                    _ => None,
                },
            }
        })
        .collect()
}
