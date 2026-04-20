//! SQLite database connector.

use async_trait::async_trait;
use sqlx::migrate::MigrateDatabase;
use sqlx::{Pool, Sqlite};

use super::DatabaseConnector;
use crate::connection_pool::ConnectionPool;
use crate::error::Error;

/// SQLite database connector.
pub struct SqliteConnector;

#[async_trait]
impl DatabaseConnector for SqliteConnector {
    async fn connect(&self, url: &str) -> Result<ConnectionPool, Error> {
        // Validate URL format
        self.validate_url(url)?;

        // Check if database exists
        if !Sqlite::database_exists(url).await.unwrap_or(false) {
            return Err(Error::InvalidDbUrl(format!(
                "SQLite database does not exist: {}",
                url
            )));
        }

        // Connect to the database
        let pool = Pool::connect(url).await?;

        Ok(ConnectionPool::Sqlite(pool))
    }

    fn validate_url(&self, url: &str) -> Result<(), Error> {
        if !url.starts_with("sqlite:") {
            return Err(Error::InvalidDbUrl(format!(
                "Invalid SQLite URL. Must start with 'sqlite:': {}",
                url
            )));
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_validate_sqlite_url() {
        let connector = SqliteConnector;

        assert!(connector.validate_url("sqlite:/path/to/db.sqlite").is_ok());
        assert!(connector.validate_url("sqlite:memory:").is_ok());
        assert!(connector.validate_url("postgresql://localhost/db").is_err());
    }
}
