//! SQLite configuration, path resolution, and sqlx execution (no UI).

pub mod catalog;
pub mod config;
pub mod constraints;
pub mod inspect;
pub mod mutations;
pub mod page;
pub mod readonly;

pub use catalog::column_catalog;
pub use config::{
    SqliteConfig, SqliteOpenOptions, SqlitePathContext, SqlitePragma, resolve_sqlite_path,
    sqlite_connect_options, sqlite_uri, sqlite3_command,
};
pub use inspect::inspect_table;
pub use mutations::{QueryColumn, delete_row, execute_sql, insert_row, update_row};
pub use page::{fetch_page, sqlite_cell_display};
pub use readonly::query_readonly;
