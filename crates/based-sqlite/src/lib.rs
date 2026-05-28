//! SQLite configuration, path resolution, and sqlx execution (no UI).

pub mod config;
pub mod mutations;

pub use config::{SqliteConfig, SqlitePathContext, resolve_sqlite_path, sqlite_connect_options};
pub use mutations::{delete_row, execute_sql, insert_row, update_row};
