use gpui_kit::component::{ActiveTheme, Theme};
use gpui_kit::{App, Hsla, SharedString};

use crate::connection::{ConnectionOrigin, ConnectionState, EngineKind};

use super::ConnectionTree;

/// Display view of a [`ConnectionState`]: exactly one holds.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) enum Link {
    Disconnected,
    Connecting,
    Connected,
    Failed(Option<String>),
}

impl Link {
    fn of(state: &ConnectionState) -> Self {
        match state {
            ConnectionState::Disconnected => Self::Disconnected,
            ConnectionState::Connecting => Self::Connecting,
            ConnectionState::Connected(_) => Self::Connected,
            ConnectionState::Failed { reason, .. } => Self::Failed(Some(reason.clone())),
        }
    }
}

#[derive(Clone)]
pub(crate) struct ConnectionRow {
    pub(crate) idx: usize,
    pub(crate) conn_label: SharedString,
    pub(crate) engine: EngineKind,
    pub(crate) state_color: Hsla,
    pub(crate) link: Link,
    pub(crate) origin: ConnectionOrigin,
}

pub(crate) enum RailItem {
    Connection(ConnectionRow),
    Divider,
}

fn connection_state_dot(state: &ConnectionState, t: &Theme) -> Hsla {
    match state {
        ConnectionState::Disconnected => t.muted_foreground.opacity(0.75),
        ConnectionState::Connecting => t.warning_foreground,
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
                link: Link::of(&entry.state),
                origin: entry.origin,
            }
        })
        .collect()
}

pub(crate) fn build_rail_items(tree: &ConnectionTree, cx: &App) -> Vec<RailItem> {
    let rows = build_connection_rows(tree, cx);
    let has_project = rows.iter().any(|r| r.origin == ConnectionOrigin::Project);
    let has_personal = rows.iter().any(|r| r.origin == ConnectionOrigin::Personal);
    let mut items = Vec::with_capacity(rows.len() + 1);
    let mut inserted_divider = false;
    for row in rows {
        if has_project
            && has_personal
            && !inserted_divider
            && row.origin == ConnectionOrigin::Personal
        {
            items.push(RailItem::Divider);
            inserted_divider = true;
        }
        items.push(RailItem::Connection(row));
    }
    items
}
