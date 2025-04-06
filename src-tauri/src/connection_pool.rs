// Copyright 2019-2023 Tauri Programme within The Commons Conservancy
// SPDX-License-Identifier: Apache-2.0
// SPDX-License-Identifier: MIT

use serde_json::Value as JsonValue;
use sqlx::migrate::MigrateDatabase;
use sqlx::{Column, Executor, Pool, Row, Sqlite};
use mongodb::{Client, Database, bson::{Document, Bson}};
use std::collections::HashMap;
use tauri::{AppHandle, Runtime};

pub enum ConnectionPool {
    Sqlite(Pool<Sqlite>),
    Mongo(Database),
}

impl ConnectionPool {
    pub(crate) async fn connect<R: Runtime>(
        conn_url: &str,
        _app: &AppHandle<R>,
    ) -> Result<Self, crate::Error> {
        match conn_url
            .split_once(':')
            .ok_or_else(|| crate::Error::InvalidDbUrl(conn_url.to_string()))?
            .0
        {
            "sqlite" => {
                // Extract the file path from the connection URL
                // let file_path = conn_url
                //     .strip_prefix("sqlite:")
                //     .ok_or_else(|| crate::Error::InvalidDbUrl(conn_url.to_string()))?;
                
                // // Check if the parent directory exists and is writable
                // let parent_dir = Path::new(file_path)
                //     .parent()
                //     .ok_or_else(|| crate::Error::InvalidDbUrl(conn_url.to_string()))?;
                
                // if !parent_dir.exists() {
                //     return Err(crate::Error::InvalidDbUrl(format!(
                //         "Parent directory does not exist: {}",
                //         parent_dir.display()
                //     )));
                // }

                // // Check if we have write permissions in the directory
                // if !fs::metadata(parent_dir)
                //     .map_err(|e| crate::Error::InvalidDbUrl(format!("Failed to check directory permissions: {}", e)))?
                //     .permissions()
                //     .readonly()
                // {
                //     return Err(crate::Error::InvalidDbUrl(format!(
                //         "No write permissions in directory: {}",
                //         parent_dir.display()
                //     )));
                // }

                if !Sqlite::database_exists(conn_url).await.unwrap_or(false) {
                    return Err(crate::Error::InvalidDbUrl(format!(
                        "Database does not exist: {}",
                        conn_url
                    )));
                }
                Ok(Self::Sqlite(Pool::connect(conn_url).await?))
            }
            "mongodb" => {                
                // Validate protocol
                if !conn_url.starts_with("mongodb://") && !conn_url.starts_with("mongodb+srv://") {
                    return Err(crate::Error::InvalidDbUrl(format!(
                        "Invalid MongoDB connection string. Must start with 'mongodb://' or 'mongodb+srv://': {}",
                        conn_url
                    )));
                }
                
                // Attempt to create a client and connect
                let client_result = Client::with_uri_str(conn_url).await;
                
                // Handle specific authentication errors
                if let Err(ref e) = client_result {
                    let error_msg = e.to_string();
                    
                    if error_msg.contains("SCRAM failure") || error_msg.contains("Authentication failed") {
                        return Err(crate::Error::MongoAuth(
                            "Authentication failed. Please check your username and password.".to_string()
                        ));
                    } else if error_msg.contains("connection refused") || error_msg.contains("timed out") {
                        return Err(crate::Error::MongoConnection(
                            "Connection refused or timed out. Please check if the MongoDB server is running and accessible.".to_string()
                        ));
                    }
                }
                
                let client = client_result?;
                
                // Extract the database name from the connection string
                let db_name = conn_url
                    .split('/')
                    .last()
                    .ok_or_else(|| crate::Error::InvalidDbUrl(format!("No database name found in connection string: {}", conn_url)))?;
                
                // Extract potential auth parameters
                let has_auth_params = db_name.contains('?');
                let clean_db_name = if has_auth_params {
                    db_name.split('?').next().unwrap_or(db_name)
                } else {
                    db_name
                };
                
                // Validate the database name isn't empty
                if clean_db_name.trim().is_empty() {
                    return Err(crate::Error::InvalidDbUrl(format!("Empty database name in connection string: {}", conn_url)));
                }
                
                // Validate that the database name doesn't contain periods or other invalid characters
                if clean_db_name.contains('.') {
                    return Err(crate::Error::InvalidDbUrl(format!("Invalid database name '{}': '.' is an invalid character in a db name", clean_db_name)));
                }

                // Other invalid characters in MongoDB database names
                let invalid_chars = ['/', '\\', ' ', '"', '$', '*', '<', '>', ':', '|', '?'];
                if let Some(c) = clean_db_name.chars().find(|&c| invalid_chars.contains(&c)) {
                    return Err(crate::Error::InvalidDbUrl(format!("Invalid database name '{}': '{}' is an invalid character in a db name", clean_db_name, c)));
                }
                
                // Create database instance
                let db = client.database(clean_db_name);
                
                // Verify connection by running a simple command
                let mut command_doc = Document::new();
                command_doc.insert("ping".to_string(), Bson::Int32(1));
                
                match db.run_command(command_doc, None).await {
                    Ok(_) => Ok(Self::Mongo(db)),
                    Err(e) => {
                        let error_msg = e.to_string();
                        if error_msg.contains("SCRAM failure") || error_msg.contains("Authentication failed") {
                            Err(crate::Error::MongoAuth(
                                "Authentication failed. Please check your username and password.".to_string()
                            ))
                        } else if error_msg.contains("authorization") {
                            Err(crate::Error::MongoAuth(
                                "Authorization failed. User may not have access to this database.".to_string()
                            ))
                        } else {
                            Err(crate::Error::Mongo(e))
                        }
                    }
                }
            }
            _ => Err(crate::Error::InvalidDbUrl(conn_url.to_string())),
        }
    }

    pub(crate) async fn close(&self) {
        match self {
            ConnectionPool::Sqlite(pool) => pool.close().await,
            ConnectionPool::Mongo(_) => (), // MongoDB client handles connection pooling internally
        }
    }

    pub(crate) async fn query(
        &self,
        query: String,
        values: Vec<JsonValue>,
    ) -> Result<Vec<HashMap<String, JsonValue>>, crate::Error> {
        match self {
            ConnectionPool::Sqlite(pool) => {
                let mut query = sqlx::query(&query);
                for value in values {
                    if value.is_null() {
                        query = query.bind(None::<JsonValue>);
                    } else if value.is_string() {
                        query = query.bind(value.as_str().unwrap().to_owned())
                    } else if let Some(number) = value.as_number() {
                        query = query.bind(number.as_f64().unwrap_or_default())
                    } else {
                        query = query.bind(value);
                    }
                }
                let rows = pool.fetch_all(query).await?;
                let mut values = Vec::new();
                for row in rows {
                    let mut value = HashMap::default();
                    for (i, column) in row.columns().iter().enumerate() {
                        let v = row.try_get_raw(i)?;
                        let v = crate::decode::sqlite::to_json(v)?;
                        value.insert(column.name().to_string(), v);
                    }
                    values.push(value);
                }
                Ok(values)
            }
            ConnectionPool::Mongo(db) => {
                // Parse the query as a MongoDB command
                let command: Document = serde_json::from_str(&query)?;
                
                // Execute the command and get the result
                let result = db.run_command(command, None).await?;
                
                // Convert the result to our expected format
                let mut values = Vec::new();
                if let Some(Bson::Array(docs)) = result.get("cursor") {
                    for doc in docs {
                        if let Bson::Document(doc) = doc {
                            let mut map = HashMap::new();
                            for (key, value) in doc {
                                let json_value = match value {
                                    Bson::Double(n) => JsonValue::Number(serde_json::Number::from_f64(*n).unwrap()),
                                    Bson::String(s) => JsonValue::String(s.clone()),
                                    Bson::Boolean(b) => JsonValue::Bool(*b),
                                    Bson::Null => JsonValue::Null,
                                    Bson::Array(arr) => {
                                        let mut json_arr = Vec::new();
                                        for item in arr {
                                            if let Bson::String(s) = item {
                                                json_arr.push(JsonValue::String(s.clone()));
                                            }
                                        }
                                        JsonValue::Array(json_arr)
                                    },
                                    _ => JsonValue::String(value.to_string()),
                                };
                                map.insert(key.clone(), json_value);
                            }
                            values.push(map);
                        }
                    }
                }
                Ok(values)
            }
        }
    }
}
