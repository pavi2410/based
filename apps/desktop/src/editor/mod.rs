//! Editor services — per-tab context, variable scoping, and schema caching.
//!
//! Engine-agnostic: all SQL editor panels (Postgres, SQLite) and future
//! LSP/autocomplete features share this layer.

pub mod pg_schema;
pub mod redis_completion;
pub mod schema_cache;
pub mod sql_completion;
pub mod sql_scope;
pub mod sqlite_schema;

pub use schema_cache::SchemaCache;
