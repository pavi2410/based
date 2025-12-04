//! Connection pool abstraction for different database types.

use mongodb::Database;
use sqlx::{Pool, Sqlite, Postgres};
use tauri::{AppHandle, Runtime};

use crate::connectors::get_connector;

/// Represents a connection pool for different database types.
pub enum ConnectionPool {
    Sqlite(Pool<Sqlite>),
    Postgres(Pool<Postgres>),
    Mongo(Database),
}

impl ConnectionPool {
    /// Connect to a database using the provided URL.
    /// 
    /// The URL scheme determines which database connector to use:
    /// - `sqlite:` - SQLite database
    /// - `postgresql:` or `postgres:` - PostgreSQL database  
    /// - `mongodb:` or `mongodb+srv:` - MongoDB database
    pub(crate) async fn connect<R: Runtime>(
        conn_url: &str,
        _app: &AppHandle<R>,
    ) -> Result<Self, crate::error::Error> {
        let connector = get_connector(conn_url)?;
        connector.connect(conn_url).await
    }

    /// Close the connection pool.
    pub(crate) async fn close(&self) {
        match self {
            ConnectionPool::Sqlite(pool) => pool.close().await,
            ConnectionPool::Postgres(pool) => pool.close().await,
            ConnectionPool::Mongo(_) => (), // MongoDB client handles connection pooling internally
        }
    }
}
