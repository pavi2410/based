// connection/ — micro-shared lifecycle layer.
//
// The ONLY cross-engine abstraction: AnyConnection enum + Connectable trait
// for open/test/close lifecycle.  Tab content reaches into engine-specific
// APIs directly; nothing from this module leaks DB-querying concerns.

pub mod environment;
pub mod lifecycle;
pub mod open;
pub mod registry;
pub mod tunnel;

pub use environment::Environment;
pub use open::{OpenedConnection, open_connection, opened_into_any};

use based_core::ContextLevel;
use serde::{Deserialize, Serialize};

use crate::db::{close_pg_pool, close_sqlite_pool};
use crate::mongodb::{MongoConfig, MongoConnection};
use crate::postgres::{PgConnection, PostgresConfig};
use crate::redis::{RedisConfig, RedisConnection};
use crate::sqlite::{SqliteConfig, SqliteConnection};

pub use based_core::categorize_connect_error;
pub use based_core::{ConnectionId, EngineKind};

/// Which based-dir (and sidebar group) a connection belongs to. Derived from the
/// id: project ids are `Project`; personal, unsaved and template ids are
/// `Personal` (user-local).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ConnectionOrigin {
    Project,
    Personal,
}

impl ConnectionOrigin {
    pub fn of(id: &ConnectionId) -> Self {
        if id.is_workspace_local() {
            Self::Personal
        } else {
            Self::Project
        }
    }
}

// ── Connection config (engine-tagged) ────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "engine", rename_all = "snake_case")]
pub enum ConnectionConfig {
    Postgres(PostgresConfig),
    MongoDB(MongoConfig),
    SQLite(SqliteConfig),
    Redis(RedisConfig),
}

impl ConnectionConfig {
    pub fn engine(&self) -> EngineKind {
        match self {
            Self::Postgres(_) => EngineKind::Postgres,
            Self::MongoDB(_) => EngineKind::MongoDB,
            Self::SQLite(_) => EngineKind::SQLite,
            Self::Redis(_) => EngineKind::Redis,
        }
    }

    pub fn label(&self) -> &str {
        match self {
            Self::Postgres(c) => &c.label,
            Self::MongoDB(c) => &c.label,
            Self::SQLite(c) => &c.label,
            Self::Redis(c) => &c.label,
        }
    }

    pub fn is_read_only(&self) -> bool {
        match self {
            Self::SQLite(c) => c.read_only,
            _ => false,
        }
    }

    pub fn with_label(mut self, label: String) -> Self {
        match &mut self {
            Self::Postgres(c) => c.label = label,
            Self::MongoDB(c) => c.label = label,
            Self::SQLite(c) => c.label = label,
            Self::Redis(c) => c.label = label,
        }
        self
    }
}

// ── Open connection (engine-tagged, no shared query interface) ────────────────

#[derive(Clone)]
pub enum AnyConnection {
    Postgres(gpui_kit::Entity<PgConnection>),
    MongoDB(gpui_kit::Entity<MongoConnection>),
    SQLite(gpui_kit::Entity<SqliteConnection>),
    Redis(gpui_kit::Entity<RedisConnection>),
}

// ── Connection state machine ──────────────────────────────────────────────────

pub enum ConnectionState {
    Disconnected,
    /// In-flight connect is tracked by `Workspace` spawn; this state is UX-only.
    Connecting,
    Connected(AnyConnection),
    Failed {
        reason: String,
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
    pub last_error: Option<String>,
    /// `[ai]` table from the connection file; `None` means never configured (AI off).
    pub ai: ContextLevel,
}

impl ConnectionEntry {
    pub fn new(id: ConnectionId, config: ConnectionConfig, tags: Vec<String>) -> Self {
        Self {
            id,
            config,
            tags,
            state: ConnectionState::Disconnected,
            last_error: None,
            ai: ContextLevel::Off,
        }
    }

    pub fn engine(&self) -> EngineKind {
        self.config.engine()
    }

    pub fn origin(&self) -> ConnectionOrigin {
        ConnectionOrigin::of(&self.id)
    }
}

// ── Connection entry events ───────────────────────────────────────────────────

pub enum ConnectionEntryEvent {}

impl gpui_kit::EventEmitter<ConnectionEntryEvent> for ConnectionEntry {}

/// A connected entry in the registry (for quit / switch-project prompts).
#[derive(Clone)]
pub struct LiveConnection {
    pub label: gpui_kit::SharedString,
    pub engine: EngineKind,
}

/// List connections in [`ConnectionState::Connected`].
pub fn live_connections(
    registry: &gpui_kit::Entity<registry::ConnectionRegistry>,
    cx: &gpui_kit::App,
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
    registry: &gpui_kit::Entity<registry::ConnectionRegistry>,
    cx: &gpui_kit::App,
) -> usize {
    live_connections(registry, cx).len()
}

/// Count project-owned connections in [`ConnectionState::Connected`].
/// Workspace-local wizard templates are excluded.
pub fn live_project_connection_count(
    registry: &gpui_kit::Entity<registry::ConnectionRegistry>,
    cx: &gpui_kit::App,
) -> usize {
    registry
        .read(cx)
        .connections()
        .iter()
        .filter(|ent| {
            let entry = ent.read(cx);
            matches!(entry.state, ConnectionState::Connected(_)) && !entry.id.is_workspace_local()
        })
        .count()
}

/// Whether the connection profile requests read-only access (SQLite enforced at open).
pub fn is_connection_read_only(
    id: &ConnectionId,
    registry: &registry::ConnectionRegistry,
    cx: &gpui_kit::App,
) -> bool {
    registry
        .get(id, cx)
        .is_some_and(|e| e.read(cx).config.is_read_only())
}

/// Close pools / clients held by a live connection handle.
pub fn close_any_connection(ac: AnyConnection, cx: &gpui_kit::App) {
    match ac {
        AnyConnection::Postgres(ent) => {
            let pool = ent.read(cx).pool.clone();
            close_pg_pool(pool);
        }
        AnyConnection::SQLite(ent) => {
            let pool = ent.read(cx).pool.clone();
            close_sqlite_pool(pool);
        }
        AnyConnection::MongoDB(_) | AnyConnection::Redis(_) => {
            // Mongo / Redis clients close when the connection entity is dropped.
        }
    }
}
