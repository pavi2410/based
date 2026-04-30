// connection/ — micro-shared lifecycle layer.
//
// The ONLY cross-engine abstraction: AnyConnection enum + Connectable trait
// for open/test/close lifecycle.  Tab content reaches into engine-specific
// APIs directly; nothing from this module leaks DB-querying concerns.

pub mod lifecycle;
pub mod persistence;
pub mod registry;

use std::time::Instant;

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

// ── Connection identity ───────────────────────────────────────────────────────

/// Stable opaque identifier for a connection, derived from the config key.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct ConnectionId(pub String);

impl ConnectionId {
    pub fn from_key(key: &str) -> Self {
        Self(key.to_string())
    }
}

impl std::fmt::Display for ConnectionId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.0)
    }
}

// ── Engine kind ───────────────────────────────────────────────────────────────

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum EngineKind {
    Postgres,
    MongoDB,
    SQLite,
}

impl EngineKind {
    pub fn short_label(self) -> &'static str {
        match self {
            Self::Postgres => "pg",
            Self::MongoDB => "mg",
            Self::SQLite => "sqlite",
        }
    }
}

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
    Connecting { since: Instant },
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
    pub state: ConnectionState,
    pub last_connected_at: Option<DateTime<Utc>>,
    pub last_error: Option<String>,
}

impl ConnectionEntry {
    pub fn new(config: ConnectionConfig) -> Self {
        let key = config.label().to_string();
        Self::with_stable_id(config, &key)
    }

    pub fn with_stable_id(config: ConnectionConfig, stable_key: &str) -> Self {
        let id = ConnectionId::from_key(stable_key);
        Self {
            id,
            config,
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

// ── Tab addressing ────────────────────────────────────────────────────────────

#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct TabId {
    pub conn: ConnectionId,
    pub kind: TabKind,
    pub payload: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TabKind {
    DataViewer,
    QueryEditor,
    SchemaInspector,
    ExplainView,
    PragmaBrowser,
    EqpViewer,
    FtsConsole,
    PipelineBuilder,
    ChangeStream,
    LiveMonitor,
}
