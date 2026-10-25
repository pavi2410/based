//! PostgreSQL configuration, connection options, query execution, and EXPLAIN parsing.

pub mod catalog;
pub mod config;
pub mod decode;
pub mod explain;
pub mod inspect;
pub mod mutations;
pub mod page;
pub mod readonly;

pub use catalog::column_catalog;
pub use config::{
    PostgresConfig, SslMode, pg_connect_options, pg_ssl_mode, postgres_uri, psql_command,
};
pub use decode::{decode_pg_cell, pg_cell_display};
pub use explain::{PlanNode, parse_pg_explain_json};
pub use inspect::inspect_table;
pub use mutations::{QueryColumn, delete_row, execute_sql, insert_row};
pub use page::fetch_page;
pub use readonly::query_readonly;
