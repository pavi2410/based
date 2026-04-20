//! PostgreSQL database connector.

use async_trait::async_trait;
use sqlx::Pool;

use super::DatabaseConnector;
use crate::connection_pool::ConnectionPool;
use crate::error::Error;

/// PostgreSQL database connector.
pub struct PostgresConnector;

#[async_trait]
impl DatabaseConnector for PostgresConnector {
    async fn connect(&self, url: &str) -> Result<ConnectionPool, Error> {
        // Validate URL format
        self.validate_url(url)?;

        // Connect to the database
        let pool = Pool::connect(url).await?;

        Ok(ConnectionPool::Postgres(pool))
    }

    fn validate_url(&self, url: &str) -> Result<(), Error> {
        if !url.starts_with("postgresql://") && !url.starts_with("postgres://") {
            return Err(Error::InvalidDbUrl(format!(
                "Invalid PostgreSQL URL. Must start with 'postgresql://' or 'postgres://': {}",
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
    fn test_validate_postgres_url() {
        let connector = PostgresConnector;

        assert!(
            connector
                .validate_url("postgresql://localhost:5432/db")
                .is_ok()
        );
        assert!(
            connector
                .validate_url("postgres://user:pass@localhost/db")
                .is_ok()
        );
        assert!(connector.validate_url("sqlite:/path/to/db").is_err());
    }
}
