//! Database connectors module.
//!
//! This module provides a trait-based abstraction for connecting to different
//! database engines, with separate implementations for SQLite, PostgreSQL, and MongoDB.

mod mongodb;
mod postgres;
mod sqlite;
mod url_parser;
mod validators;

pub use self::mongodb::MongoConnector;
pub use self::postgres::PostgresConnector;
pub use self::sqlite::SqliteConnector;
pub use url_parser::{DatabaseUrl, parse_database_url};

use crate::connection_pool::ConnectionPool;
use crate::error::Error;
use async_trait::async_trait;

/// Trait for database-specific connection logic.
/// Each database engine implements this trait to handle its own
/// URL parsing, validation, and connection establishment.
#[async_trait]
pub trait DatabaseConnector: Send + Sync {
    /// Connect to the database using the provided URL.
    async fn connect(&self, url: &str) -> Result<ConnectionPool, Error>;

    /// Validate the connection URL format without connecting.
    fn validate_url(&self, url: &str) -> Result<(), Error>;
}

/// Get the appropriate connector for a database URL.
pub fn get_connector(url: &str) -> Result<Box<dyn DatabaseConnector>, Error> {
    let db_url = parse_database_url(url)?;

    match db_url {
        DatabaseUrl::Sqlite(_) => Ok(Box::new(SqliteConnector)),
        DatabaseUrl::Postgres(_) => Ok(Box::new(PostgresConnector)),
        DatabaseUrl::Mongo { .. } => Ok(Box::new(MongoConnector)),
    }
}
