// Copyright 2019-2023 Tauri Programme within The Commons Conservancy
// SPDX-License-Identifier: Apache-2.0
// SPDX-License-Identifier: MIT

use serde_json::Value as JsonValue;
use sqlx::migrate::MigrateDatabase;
use sqlx::{Column, Executor, Pool, Row, Sqlite};
use mongodb::{Client, Database, bson::{Document, Bson}};
use std::collections::HashMap;
use tauri::{AppHandle, Runtime};

pub enum DbPool {
    Sqlite(Pool<Sqlite>),
    Mongo(Database),
}

impl DbPool {
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
                if !Sqlite::database_exists(conn_url).await.unwrap_or(false) {
                    // TODO: maybe throw a DatabaseNotExists error?
                    Sqlite::create_database(conn_url).await?;
                }
                Ok(Self::Sqlite(Pool::connect(conn_url).await?))
            }
            "mongodb" => {
                let client = Client::with_uri_str(conn_url).await?;
                let db_name = conn_url
                    .split('/')
                    .last()
                    .ok_or_else(|| crate::Error::InvalidDbUrl(conn_url.to_string()))?;
                let db = client.database(db_name);
                Ok(Self::Mongo(db))
            }
            _ => Err(crate::Error::InvalidDbUrl(conn_url.to_string())),
        }
    }

    pub(crate) async fn close(&self) {
        match self {
            DbPool::Sqlite(pool) => pool.close().await,
            DbPool::Mongo(_) => (), // MongoDB client handles connection pooling internally
        }
    }

    pub(crate) async fn query(
        &self,
        query: String,
        values: Vec<JsonValue>,
    ) -> Result<Vec<HashMap<String, JsonValue>>, crate::Error> {
        match self {
            DbPool::Sqlite(pool) => {
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
            DbPool::Mongo(db) => {
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
