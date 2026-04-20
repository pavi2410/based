//! Error taxonomy for the Based backend.
//!
//! The goal is to keep user-facing errors as a **typed, structured**
//! enum (not a string blob) without the frontend having to know every
//! leaf error type from sqlx / mongodb / toml / etc.
//!
//! Scoping:
//! - [`ConnectError`] — anything that happens when opening a connection,
//!   reading `config.toml`, or resolving secrets. The user almost always
//!   needs to fix the project file.
//! - [`QueryError`] — anything that happens when executing a query on a
//!   previously-good connection. Usually retryable.
//! - [`ProjectError`] — project-file-level IO/parse errors, used by the
//!   project_commands module before we even know about a connection.
//! - [`AppError`] — the union returned at the Tauri/IPC boundary. The
//!   frontend pattern-matches on its `kind`.
//!
//! Downstream code can still keep using the historical [`Error`] alias
//! (which is just `AppError`) until every command is migrated.

use serde::{Serialize, Serializer};

/// Error while opening a connection or reading project config.
#[derive(Debug, thiserror::Error)]
pub enum ConnectError {
    #[error("invalid connection url: {0}")]
    InvalidDbUrl(String),
    #[error("connection not found: {0}")]
    NotFound(String),
    #[error("mongodb authentication error: {0}")]
    MongoAuth(String),
    #[error("mongodb connection error: {0}")]
    MongoConnection(String),
    #[error(transparent)]
    Sql(#[from] sqlx::Error),
    #[error(transparent)]
    Mongo(#[from] mongodb::error::Error),
}

/// Error while executing a query or browsing a table on an already-opened
/// connection.
#[derive(Debug, thiserror::Error)]
pub enum QueryError {
    #[error("database {0} not loaded")]
    DatabaseNotLoaded(String),
    #[error("unsupported datatype: {0}")]
    UnsupportedDatatype(String),
    #[error(transparent)]
    Sql(#[from] sqlx::Error),
    #[error(transparent)]
    Mongo(#[from] mongodb::error::Error),
    #[error(transparent)]
    Json(#[from] serde_json::Error),
}

/// Error when initialising / reading / writing a `.based/` project on
/// disk. Separate from `ConnectError` so we can surface a different UX
/// ("project is broken" vs "this connection is broken").
#[derive(Debug, thiserror::Error)]
pub enum ProjectError {
    #[error("io error: {0}")]
    Io(#[from] std::io::Error),
    #[error("toml parse error: {0}")]
    TomlParse(#[from] toml::de::Error),
    #[error("toml serialize error: {0}")]
    TomlSerialize(#[from] toml::ser::Error),
    #[error("yaml parse error: {0}")]
    YamlParse(#[from] serde_yaml::Error),
    #[error("project not initialized at: {0}")]
    NotInitialized(String),
    #[error("invalid frontmatter: {0}")]
    InvalidFrontmatter(String),
}

/// Backwards-compatible umbrella error returned at the Tauri boundary.
///
/// Every command currently returns `Result<T, Error>` which serializes as
/// a string via the `Serialize` impl below. Phase 1 will flip individual
/// commands to the scoped errors above and the frontend will learn to
/// switch on `kind`, but keeping the alias here means we can migrate
/// incrementally without breaking the IPC surface.
#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error(transparent)]
    Sql(#[from] sqlx::Error),
    #[error(transparent)]
    Mongo(#[from] mongodb::error::Error),
    #[error(transparent)]
    Json(#[from] serde_json::Error),
    #[error("invalid connection url: {0}")]
    InvalidDbUrl(String),
    #[error("database {0} not loaded")]
    DatabaseNotLoaded(String),
    #[error("unsupported datatype: {0}")]
    UnsupportedDatatype(String),
    #[error("mongodb authentication error: {0}")]
    MongoAuth(String),
    #[error("mongodb connection error: {0}")]
    MongoConnection(String),
    #[error("connection not found: {0}")]
    ConnectionNotFound(String),
    #[error(transparent)]
    Project(#[from] ProjectError),
}

impl From<String> for Error {
    fn from(s: String) -> Self {
        Error::ConnectionNotFound(s)
    }
}

impl From<ConnectError> for Error {
    fn from(e: ConnectError) -> Self {
        match e {
            ConnectError::InvalidDbUrl(s) => Error::InvalidDbUrl(s),
            ConnectError::NotFound(s) => Error::ConnectionNotFound(s),
            ConnectError::MongoAuth(s) => Error::MongoAuth(s),
            ConnectError::MongoConnection(s) => Error::MongoConnection(s),
            ConnectError::Sql(e) => Error::Sql(e),
            ConnectError::Mongo(e) => Error::Mongo(e),
        }
    }
}

impl From<QueryError> for Error {
    fn from(e: QueryError) -> Self {
        match e {
            QueryError::DatabaseNotLoaded(s) => Error::DatabaseNotLoaded(s),
            QueryError::UnsupportedDatatype(s) => Error::UnsupportedDatatype(s),
            QueryError::Sql(e) => Error::Sql(e),
            QueryError::Mongo(e) => Error::Mongo(e),
            QueryError::Json(e) => Error::Json(e),
        }
    }
}

impl Serialize for Error {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        serializer.serialize_str(self.to_string().as_ref())
    }
}

// Error serializes to string at the IPC boundary, so specta sees it as String.
impl specta::Type for Error {
    fn inline(_: &mut specta::TypeMap, _: specta::Generics) -> specta::DataType {
        String::inline(&mut specta::TypeMap::default(), specta::Generics::Definition)
    }
}
