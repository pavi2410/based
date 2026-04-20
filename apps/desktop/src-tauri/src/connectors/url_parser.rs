//! URL parsing for database connection strings.

use crate::error::Error;

/// Parsed database URL with type-specific information.
#[derive(Debug, Clone)]
pub enum DatabaseUrl {
    /// SQLite connection: `sqlite:/path/to/db.sqlite`
    Sqlite(String),

    /// PostgreSQL connection: `postgresql://user:pass@host:port/db`
    Postgres(String),

    /// MongoDB connection with extracted database name
    Mongo {
        /// Full connection URI
        uri: String,
        /// Extracted database name (cleaned of query params)
        database: String,
    },
}

impl DatabaseUrl {
    /// Get the original URL string.
    pub fn as_str(&self) -> &str {
        match self {
            DatabaseUrl::Sqlite(url) => url,
            DatabaseUrl::Postgres(url) => url,
            DatabaseUrl::Mongo { uri, .. } => uri,
        }
    }
}

/// Parse a connection URL and determine its database type.
pub fn parse_database_url(url: &str) -> Result<DatabaseUrl, Error> {
    let scheme = url
        .split_once(':')
        .map(|(s, _)| s)
        .ok_or_else(|| Error::InvalidDbUrl(format!("No scheme found in URL: {}", url)))?;

    match scheme {
        "sqlite" => Ok(DatabaseUrl::Sqlite(url.to_string())),

        "postgresql" | "postgres" => Ok(DatabaseUrl::Postgres(url.to_string())),

        "mongodb" | "mongodb+srv" => {
            let database = extract_mongo_database(url)?;
            Ok(DatabaseUrl::Mongo {
                uri: url.to_string(),
                database,
            })
        }

        _ => Err(Error::InvalidDbUrl(format!(
            "Unsupported database scheme: {}",
            scheme
        ))),
    }
}

/// Extract the database name from a MongoDB connection string.
fn extract_mongo_database(url: &str) -> Result<String, Error> {
    // MongoDB URL format: mongodb://[user:pass@]host[:port]/database[?options]
    // or: mongodb+srv://[user:pass@]host/database[?options]

    let db_name = url.split('/').last().ok_or_else(|| {
        Error::InvalidDbUrl(format!(
            "No database name found in MongoDB connection string: {}",
            url
        ))
    })?;

    // Remove query parameters if present
    let clean_db_name = if db_name.contains('?') {
        db_name.split('?').next().unwrap_or(db_name)
    } else {
        db_name
    };

    if clean_db_name.trim().is_empty() {
        return Err(Error::InvalidDbUrl(format!(
            "Empty database name in MongoDB connection string: {}",
            url
        )));
    }

    Ok(clean_db_name.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_sqlite_url() {
        let url = "sqlite:/path/to/db.sqlite";
        let result = parse_database_url(url).unwrap();
        assert!(matches!(result, DatabaseUrl::Sqlite(_)));
    }

    #[test]
    fn test_parse_postgres_url() {
        let url = "postgresql://user:pass@localhost:5432/mydb";
        let result = parse_database_url(url).unwrap();
        assert!(matches!(result, DatabaseUrl::Postgres(_)));
    }

    #[test]
    fn test_parse_mongo_url() {
        let url = "mongodb://localhost:27017/mydb";
        let result = parse_database_url(url).unwrap();
        match result {
            DatabaseUrl::Mongo { database, .. } => {
                assert_eq!(database, "mydb");
            }
            _ => panic!("Expected MongoDB URL"),
        }
    }

    #[test]
    fn test_parse_mongo_url_with_params() {
        let url = "mongodb://localhost:27017/mydb?authSource=admin";
        let result = parse_database_url(url).unwrap();
        match result {
            DatabaseUrl::Mongo { database, .. } => {
                assert_eq!(database, "mydb");
            }
            _ => panic!("Expected MongoDB URL"),
        }
    }

    #[test]
    fn test_parse_invalid_scheme() {
        let url = "mysql://localhost/db";
        let result = parse_database_url(url);
        assert!(result.is_err());
    }
}
