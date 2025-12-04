//! Validation functions for database connection parameters.

use crate::error::Error;

/// Validate a MongoDB database name.
/// 
/// MongoDB database names have specific restrictions:
/// - Cannot be empty
/// - Cannot contain: / \ . " $ * < > : | ?
/// - Cannot contain spaces
pub fn validate_mongo_database_name(name: &str) -> Result<(), Error> {
    if name.trim().is_empty() {
        return Err(Error::InvalidDbUrl(
            "MongoDB database name cannot be empty".to_string()
        ));
    }
    
    // Period is invalid
    if name.contains('.') {
        return Err(Error::InvalidDbUrl(format!(
            "Invalid MongoDB database name '{}': '.' is an invalid character",
            name
        )));
    }
    
    // Other invalid characters
    let invalid_chars = ['/', '\\', ' ', '"', '$', '*', '<', '>', ':', '|', '?'];
    if let Some(c) = name.chars().find(|&c| invalid_chars.contains(&c)) {
        return Err(Error::InvalidDbUrl(format!(
            "Invalid MongoDB database name '{}': '{}' is an invalid character",
            name, c
        )));
    }
    
    Ok(())
}

/// Validate a MongoDB connection URL format.
pub fn validate_mongo_url(url: &str) -> Result<(), Error> {
    if !url.starts_with("mongodb://") && !url.starts_with("mongodb+srv://") {
        return Err(Error::InvalidDbUrl(format!(
            "Invalid MongoDB connection string. Must start with 'mongodb://' or 'mongodb+srv://': {}",
            url
        )));
    }
    Ok(())
}

/// Classify a MongoDB error into a more specific error type.
pub fn classify_mongo_error(error_msg: &str) -> Option<Error> {
    if error_msg.contains("SCRAM failure") || error_msg.contains("Authentication failed") {
        Some(Error::MongoAuth(
            "Authentication failed. Please check your username and password.".to_string()
        ))
    } else if error_msg.contains("connection refused") || error_msg.contains("timed out") {
        Some(Error::MongoConnection(
            "Connection refused or timed out. Please check if the MongoDB server is running and accessible.".to_string()
        ))
    } else if error_msg.contains("authorization") {
        Some(Error::MongoAuth(
            "Authorization failed. User may not have access to this database.".to_string()
        ))
    } else {
        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_valid_mongo_db_name() {
        assert!(validate_mongo_database_name("mydb").is_ok());
        assert!(validate_mongo_database_name("my_db").is_ok());
        assert!(validate_mongo_database_name("my-db").is_ok());
    }

    #[test]
    fn test_invalid_mongo_db_name_empty() {
        assert!(validate_mongo_database_name("").is_err());
        assert!(validate_mongo_database_name("   ").is_err());
    }

    #[test]
    fn test_invalid_mongo_db_name_period() {
        assert!(validate_mongo_database_name("my.db").is_err());
    }

    #[test]
    fn test_invalid_mongo_db_name_special_chars() {
        assert!(validate_mongo_database_name("my/db").is_err());
        assert!(validate_mongo_database_name("my db").is_err());
        assert!(validate_mongo_database_name("my$db").is_err());
    }

    #[test]
    fn test_valid_mongo_url() {
        assert!(validate_mongo_url("mongodb://localhost:27017/db").is_ok());
        assert!(validate_mongo_url("mongodb+srv://cluster.mongodb.net/db").is_ok());
    }

    #[test]
    fn test_invalid_mongo_url() {
        assert!(validate_mongo_url("http://localhost:27017/db").is_err());
        assert!(validate_mongo_url("mongo://localhost:27017/db").is_err());
    }
}
