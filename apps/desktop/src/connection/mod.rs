// connection/ — micro-shared lifecycle layer.
//
// The ONLY cross-engine abstraction: AnyConnection enum + Connectable trait
// for open/test/close lifecycle.  Tab content reaches into engine-specific
// APIs directly; nothing from this module leaks DB-querying concerns.

pub mod lifecycle;
pub mod open;
pub mod persistence;
pub mod registry;

pub use open::{OpenedConnection, open_connection, opened_into_any};

use std::time::Instant;

use serde::{Deserialize, Serialize};
use time::OffsetDateTime;

pub use based_core::categorize_connect_error;
pub use based_core::{ConnectionId, EngineKind};

// ── Connection config (engine-tagged) ────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "engine", rename_all = "snake_case")]
pub enum ConnectionConfig {
    Postgres(crate::postgres::PostgresConfig),
    MongoDB(crate::mongodb::MongoConfig),
    SQLite(crate::sqlite::SqliteConfig),
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
}

// ── Open connection (engine-tagged, no shared query interface) ────────────────

#[derive(Clone)]
pub enum AnyConnection {
    Postgres(gpui::Entity<crate::postgres::PgConnection>),
    MongoDB(gpui::Entity<crate::mongodb::MongoConnection>),
    SQLite(gpui::Entity<crate::sqlite::SqliteConnection>),
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
        let id = ConnectionId::from_key(stable_key);
        Self {
            id,
            config,
            tags,
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

/// Count connections in [`ConnectionState::Connected`].
pub fn live_connection_count(
    registry: &gpui::Entity<registry::ConnectionRegistry>,
    cx: &gpui::App,
) -> usize {
    registry
        .read(cx)
        .connections()
        .iter()
        .filter(|e| matches!(e.read(cx).state, ConnectionState::Connected(_)))
        .count()
}

/// Close pools / clients held by a live connection handle.
pub fn close_any_connection(ac: AnyConnection, cx: &gpui::App) {
    match ac {
        AnyConnection::Postgres(ent) => {
            let pool = ent.read(cx).pool.clone();
            crate::db::close_pg_pool(pool);
        }
        AnyConnection::SQLite(ent) => {
            let pool = ent.read(cx).pool.clone();
            crate::db::close_sqlite_pool(pool);
        }
        AnyConnection::MongoDB(_) => {
            // Mongo client closes when the connection entity is dropped.
        }
    }
}
