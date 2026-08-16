//! SQLite configuration, path resolution, and sqlx execution (no UI).

pub mod config;
pub mod mutations;

pub use config::{
    SqliteConfig, SqliteOpenOptions, SqlitePathContext, SqlitePragma, resolve_sqlite_path,
    sqlite_connect_options, sqlite_uri, sqlite3_command,
};
pub use mutations::{QueryColumn, delete_row, execute_sql, insert_row, update_row};
