//! MongoDB database connector.

use async_trait::async_trait;
use mongodb::{
    Client,
    bson::{Bson, Document},
};

use super::url_parser::parse_database_url;
use super::validators::{classify_mongo_error, validate_mongo_database_name, validate_mongo_url};
use super::DatabaseConnector;
use crate::connection_pool::ConnectionPool;
use crate::error::Error;

/// MongoDB database connector.
pub struct MongoConnector;

#[async_trait]
impl DatabaseConnector for MongoConnector {
    async fn connect(&self, url: &str) -> Result<ConnectionPool, Error> {
        // Validate URL format
        self.validate_url(url)?;
        
        // Parse URL to extract database name
        let db_url = parse_database_url(url)?;
        let database_name = match db_url {
            super::url_parser::DatabaseUrl::Mongo { database, .. } => database,
            _ => return Err(Error::InvalidDbUrl("Expected MongoDB URL".to_string())),
        };
        
        // Validate database name
        validate_mongo_database_name(&database_name)?;
        
        // Attempt to create client
        let client = match Client::with_uri_str(url).await {
            Ok(client) => client,
            Err(e) => {
                let error_msg = e.to_string();
                if let Some(classified_error) = classify_mongo_error(&error_msg) {
                    return Err(classified_error);
                }
                return Err(Error::Mongo(e));
            }
        };
        
        // Get database handle
        let db = client.database(&database_name);
        
        // Verify connection with ping
        let mut ping_cmd = Document::new();
        ping_cmd.insert("ping".to_string(), Bson::Int32(1));
        
        match db.run_command(ping_cmd, None).await {
            Ok(_) => Ok(ConnectionPool::Mongo(db)),
            Err(e) => {
                let error_msg = e.to_string();
                if let Some(classified_error) = classify_mongo_error(&error_msg) {
                    Err(classified_error)
                } else {
                    Err(Error::Mongo(e))
                }
            }
        }
    }
    
    fn validate_url(&self, url: &str) -> Result<(), Error> {
        validate_mongo_url(url)
    }
}
