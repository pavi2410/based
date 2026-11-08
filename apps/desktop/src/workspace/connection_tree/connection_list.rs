use gpui_kit::component::{ActiveTheme, Theme};
use gpui_kit::{App, Hsla, SharedString};

use crate::connection::{
    ConnectionConfig, ConnectionOrigin, ConnectionState, EngineKind, Environment,
};

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
    pub(crate) environment: Option<Environment>,
    /// Where it lives: `host:port/db`, a file path, or a credential-free URL host.
    pub(crate) detail: SharedString,
    pub(crate) read_only: bool,
}

pub(crate) enum RailItem {
    Connection(ConnectionRow),
    Divider,
}

/// One-line, credential-free description of where a connection points.
fn config_detail(config: &ConnectionConfig) -> String {
    fn host_of(url: &str) -> String {
        let rest = url.split("://").nth(1).unwrap_or(url);
        rest.rsplit('@').next().unwrap_or(rest).to_string()
    }
    match config {
        ConnectionConfig::Postgres(c) => format!("{}:{}/{}", c.host, c.port, c.database),
        ConnectionConfig::MongoDB(c) => host_of(&c.uri),
        ConnectionConfig::Redis(c) => host_of(&c.url),
        ConnectionConfig::SQLite(c) => c.path.display().to_string(),
    }
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
                environment: Environment::from_tags(&entry.tags),
                detail: config_detail(&entry.config).into(),
                read_only: entry.config.is_read_only(),
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
