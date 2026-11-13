//! SQLite configuration, path resolution, and sqlx execution (no UI).

pub mod catalog;
pub mod config;
pub mod constraints;
mod error;
pub mod inspect;
pub mod mutations;
mod objects;
pub mod page;
pub mod readonly;

pub use catalog::column_catalog;
pub use config::{
    JournalMode, SqliteConfig, SqliteOpenOptions, SqlitePathContext, SqlitePragma, Synchronous,
    UnknownPragmaValue, resolve_sqlite_path, sqlite_connect_options, sqlite_uri, sqlite3_command,
};
pub use error::connect_error;
pub use inspect::table_spec;
pub use mutations::{delete_row, execute_sql, insert_row, update_row};
pub use objects::{list_objects, list_relations};
pub use page::{fetch_page, sqlite_cell};
pub use readonly::query_readonly;
