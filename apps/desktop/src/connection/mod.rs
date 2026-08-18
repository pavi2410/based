// connection/ — micro-shared lifecycle layer.
//
// The ONLY cross-engine abstraction: AnyConnection enum + Connectable trait
// for open/test/close lifecycle.  Tab content reaches into engine-specific
// APIs directly; nothing from this module leaks DB-querying concerns.

pub mod descriptor;
pub mod lifecycle;
pub mod open;
pub mod registry;

pub use descriptor::EngineRegistry;

pub use open::{OpenedConnection, open_connection, opened_into_any};

use std::time::Instant;

use serde::{Deserialize, Serialize};
use time::OffsetDateTime;

use crate::db::{close_pg_pool, close_sqlite_pool};
use crate::mongodb::{MongoConfig, MongoConnection};
use crate::postgres::{PgConnection, PostgresConfig};
use crate::sqlite::{SqliteConfig, SqliteConnection};

pub use based_core::categorize_connect_error;
pub use based_core::{ConnectionId, EngineKind};

/// Which based-dir a connection was loaded from.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum ConnectionOrigin {
    #[default]
    Project,
    Personal,
}

// ── Connection config (engine-tagged) ────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "engine", rename_all = "snake_case")]
pub enum ConnectionConfig {
    Postgres(PostgresConfig),
    MongoDB(MongoConfig),
    SQLite(SqliteConfig),
}

impl ConnectionConfig {
    pub fn engine(&self) -> EngineKind {
        match self {
            Self::Postgres(_) => EngineKind::Postgres,
            Self::MongoDB(_) => EngineKind::MongoDB,
            Self::SQLite(_) => EngineKind::SQLite,
        }
    }

    pub fn label(&self) -> &str {
        match self {
            Self::Postgres(c) => &c.label,
            Self::MongoDB(c) => &c.label,
            Self::SQLite(c) => &c.label,
        }
    }

    pub fn is_read_only(&self) -> bool {
        match self {
            Self::SQLite(c) => c.read_only,
            _ => false,
        }
    }
}

// ── Open connection (engine-tagged, no shared query interface) ────────────────

#[derive(Clone)]
pub enum AnyConnection {
    Postgres(gpui::Entity<PgConnection>),
    MongoDB(gpui::Entity<MongoConnection>),
    SQLite(gpui::Entity<SqliteConnection>),
}

// ── Connection state machine ──────────────────────────────────────────────────

pub enum ConnectionState {
    Disconnected,
    /// In-flight connect is tracked by `Workspace` spawn; this state is UX-only.
    Connecting {
        since: Instant,
    },
    Connected(AnyConnection),
    Failed {
        reason: String,
        attempted_at: Instant,
    },
}

impl ConnectionState {
    pub fn label(&self) -> &'static str {
        match self {
            Self::Disconnected => "disconnected",
            Self::Connecting { .. } => "connecting",
            Self::Connected(_) => "connected",
            Self::Failed { .. } => "failed",
        }
    }
}

// ── Connection entry (live entity) ───────────────────────────────────────────

pub struct ConnectionEntry {
    pub id: ConnectionId,
    pub config: ConnectionConfig,
    pub tags: Vec<String>,
    pub origin: ConnectionOrigin,
    pub state: ConnectionState,
    pub last_connected_at: Option<OffsetDateTime>,
    pub last_error: Option<String>,
}

impl ConnectionEntry {
    pub fn new(config: ConnectionConfig) -> Self {
        let key = config.label().to_string();
        Self::with_stable_id(config, &key)
    }

    pub fn with_stable_id(config: ConnectionConfig, stable_key: &str) -> Self {
        Self::with_stable_id_and_tags(config, stable_key, vec![])
    }

    pub fn with_stable_id_and_tags(
        config: ConnectionConfig,
        stable_key: &str,
        tags: Vec<String>,
    ) -> Self {
        Self::with_origin(config, stable_key, tags, ConnectionOrigin::Project)
    }

    pub fn with_origin(
        config: ConnectionConfig,
        stable_key: &str,
        tags: Vec<String>,
        origin: ConnectionOrigin,
    ) -> Self {
        let id = ConnectionId::from_key(stable_key);
        Self {
            id,
            config,
            tags,
            origin,
            state: ConnectionState::Disconnected,
            last_connected_at: None,
            last_error: None,
        }
    }

    pub fn engine(&self) -> EngineKind {
        self.config.engine()
    }
}

// ── Connection entry events ───────────────────────────────────────────────────

pub enum ConnectionEntryEvent {}

impl gpui::EventEmitter<ConnectionEntryEvent> for ConnectionEntry {}

/// A connected entry in the registry (for quit / switch-project prompts).
#[derive(Clone)]
pub struct LiveConnection {
    pub label: gpui::SharedString,
    pub engine: EngineKind,
}

/// List connections in [`ConnectionState::Connected`].
pub fn live_connections(
    registry: &gpui::Entity<registry::ConnectionRegistry>,
    cx: &gpui::App,
) -> Vec<LiveConnection> {
    registry
        .read(cx)
        .connections()
        .iter()
        .filter_map(|ent| {
            let entry = ent.read(cx);
            matches!(entry.state, ConnectionState::Connected(_)).then(|| LiveConnection {
                label: entry.config.label().into(),
                engine: entry.config.engine(),
            })
        })
        .collect()
}

/// Count connections in [`ConnectionState::Connected`].
pub fn live_connection_count(
    registry: &gpui::Entity<registry::ConnectionRegistry>,
    cx: &gpui::App,
) -> usize {
    live_connections(registry, cx).len()
}

/// Count project-owned connections in [`ConnectionState::Connected`].
/// Workspace-local wizard templates are excluded.
pub fn live_project_connection_count(
    registry: &gpui::Entity<registry::ConnectionRegistry>,
    cx: &gpui::App,
) -> usize {
    registry
        .read(cx)
        .connections()
        .iter()
        .filter(|ent| {
            let entry = ent.read(cx);
            matches!(entry.state, ConnectionState::Connected(_))
                && !entry.id.is_workspace_local()
                && entry.origin != ConnectionOrigin::Personal
        })
        .count()
}

/// Whether the connection profile requests read-only access (SQLite enforced at open).
pub fn is_connection_read_only(
    id: &ConnectionId,
    registry: &registry::ConnectionRegistry,
    cx: &gpui::App,
) -> bool {
    registry
        .get(id, cx)
        .is_some_and(|e| e.read(cx).config.is_read_only())
}

/// Close pools / clients held by a live connection handle.
pub fn close_any_connection(ac: AnyConnection, cx: &gpui::App) {
    match ac {
        AnyConnection::Postgres(ent) => {
            let pool = ent.read(cx).pool.clone();
            close_pg_pool(pool);
        }
        AnyConnection::SQLite(ent) => {
            let pool = ent.read(cx).pool.clone();
            close_sqlite_pool(pool);
        }
        AnyConnection::MongoDB(_) => {
            // Mongo client closes when the connection entity is dropped.
        }
    }
}
