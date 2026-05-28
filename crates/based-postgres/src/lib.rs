//! PostgreSQL configuration, connection options, query execution, and EXPLAIN parsing.

pub mod config;
pub mod explain;
pub mod mutations;

pub use config::{PostgresConfig, SslMode, pg_connect_options, pg_ssl_mode};
pub use explain::{PlanNode, parse_pg_explain_json};
pub use mutations::{delete_row, execute_sql, insert_row};
