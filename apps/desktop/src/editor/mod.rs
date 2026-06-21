//! Editor services — per-tab context, variable scoping, and schema caching.
//!
//! Engine-agnostic: all SQL editor panels (Postgres, SQLite) and future
//! LSP/autocomplete features share this layer.

pub mod context;
pub mod schema_cache;
pub mod sql_completion;
pub mod sqlite_schema;
pub mod variable_scope;

pub use context::EditorContext;
pub use schema_cache::SchemaCache;
pub use variable_scope::VariableScope;
