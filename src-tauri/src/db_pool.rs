// Copyright 2019-2023 Tauri Programme within The Commons Conservancy
// SPDX-License-Identifier: Apache-2.0
// SPDX-License-Identifier: MIT

use serde_json::Value as JsonValue;
use sqlx::migrate::MigrateDatabase;
use sqlx::{Column, Executor, Pool, Row, Sqlite};
use std::collections::HashMap;
use tauri::{AppHandle, Runtime};

pub enum DbPool {
    Sqlite(Pool<Sqlite>),
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
            _ => Err(crate::Error::InvalidDbUrl(conn_url.to_string())),
        }
    }

    pub(crate) async fn close(&self) {
        match self {
            DbPool::Sqlite(pool) => pool.close().await,
        }
    }

    pub(crate) async fn query(
        &self,
        _query: String,
        _values: Vec<JsonValue>,
    ) -> Result<Vec<HashMap<String, JsonValue>>, crate::Error> {
        Ok(match self {
            DbPool::Sqlite(pool) => {
                let mut query = sqlx::query(&_query);
                for value in _values {
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
                values
            }
        })
    }
}