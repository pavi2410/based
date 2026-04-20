//! Thin Tauri command layer over `engine/`.
//!
//! Every command here does the same three things:
//!   1. Ensure a connection is open (creating one from `config.toml` if
//!      needed)
//!   2. Grab the pool from the `ConnectionRegistry`
//!   3. Delegate the actual work to the right `engine::*` module
//!
//! Keeping the per-engine SQL / BSON code out of this file means adding
//! a new command is a one-file change; and adding a fourth engine is a
//! one-module change (Phase 1 todo 1 scope).

use crate::connection_id::{ConnectionInfo, ConnectionRegistry};
use crate::connection_pool::ConnectionPool;
use crate::engine::mongo::MongoDBCollection;
use crate::engine::postgres::{PostgresSchema, PostgresTable};
use crate::engine::sqlite::SQLiteObject;
use crate::engine::{self, BrowseOptions, QueryResult};
use crate::error::Error;
use crate::project_commands::read_project_config;
use crate::project_types::{ConnectionConfig, Engine};
use std::collections::HashMap;
use tauri::State;

// ---------------------------------------------------------------------------
// Connection lifecycle commands
// ---------------------------------------------------------------------------

/// Build the raw connection string for a given connection config, using
/// the project's `.env` to resolve `${env:...}` secrets.
fn build_connection_string(
    conn_config: &ConnectionConfig,
    project_path: &str,
    env_vars: &HashMap<String, String>,
) -> Result<String, Error> {
    match conn_config.engine {
        Engine::Sqlite => {
            let file = conn_config.file.as_ref().ok_or_else(|| {
                Error::InvalidDbUrl("SQLite connection missing 'file' field".to_string())
            })?;

            let absolute_path = if std::path::Path::new(&file).is_relative() {
                std::path::Path::new(project_path)
                    .join(file)
                    .to_string_lossy()
                    .to_string()
            } else {
                file.clone()
            };
            Ok(format!("sqlite:{}", absolute_path))
        }
        Engine::MongoDB => {
            let url_secret = conn_config.url.as_ref().ok_or_else(|| {
                Error::InvalidDbUrl("MongoDB connection missing 'url' field".to_string())
            })?;
            url_secret
                .resolve(env_vars)
                .map_err(|e| Error::InvalidDbUrl(format!("Failed to resolve MongoDB URL: {}", e)))
        }
        Engine::Postgres => {
            if let Some(url_secret) = &conn_config.url {
                return url_secret.resolve(env_vars).map_err(|e| {
                    Error::InvalidDbUrl(format!("Failed to resolve PostgreSQL URL: {}", e))
                });
            }

            let host = conn_config.host.as_ref().ok_or_else(|| {
                Error::InvalidDbUrl(
                    "PostgreSQL connection missing 'host' field (or use 'url' for connection string)"
                        .to_string(),
                )
            })?;
            let port = conn_config.port.unwrap_or(5432);
            let database = conn_config.database.as_ref().ok_or_else(|| {
                Error::InvalidDbUrl("PostgreSQL connection missing 'database' field".to_string())
            })?;
            let username = conn_config.username.as_ref().ok_or_else(|| {
                Error::InvalidDbUrl("PostgreSQL connection missing 'username' field".to_string())
            })?;

            let password = if let Some(pass_secret) = &conn_config.password {
                pass_secret.resolve(env_vars).map_err(|e| {
                    Error::InvalidDbUrl(format!("Failed to resolve PostgreSQL password: {}", e))
                })?
            } else {
                String::new()
            };

            let ssl_mode = if conn_config.ssl.unwrap_or(false) {
                "require"
            } else {
                "disable"
            };
            Ok(format!(
                "postgresql://{}:{}@{}:{}/{}?sslmode={}",
                username, password, host, port, database, ssl_mode
            ))
        }
    }
}

fn load_project_env(project_path: &str) -> HashMap<String, String> {
    crate::variables::load_env_file(project_path).unwrap_or_default()
}

/// Ensure a connection exists, creating it on demand. Returns the
/// registry-assigned connection ID.
async fn ensure_connection(
    project_path: &str,
    conn_key: &str,
    registry: &State<'_, ConnectionRegistry>,
    app: tauri::AppHandle,
) -> Result<String, Error> {
    if registry.contains_by_key(project_path, conn_key).await {
        let id = ConnectionRegistry::get_id(project_path, conn_key);
        return Ok(id.to_string());
    }

    let config = read_project_config(project_path.to_string())
        .await
        .map_err(Error::InvalidDbUrl)?;

    let conn_config = config
        .connection
        .get(conn_key)
        .ok_or_else(|| Error::InvalidDbUrl(format!("Connection key not found: {}", conn_key)))?;

    let env_vars = load_project_env(project_path);
    let conn_string = build_connection_string(conn_config, project_path, &env_vars)?;
    let pool = ConnectionPool::connect(&conn_string, &app).await?;

    let id = registry
        .register(
            project_path.to_string(),
            conn_key.to_string(),
            conn_config.engine.clone(),
            conn_config.label.clone(),
            pool,
        )
        .await;

    Ok(id.to_string())
}

/// Mismatched-engine error helper so each command has a one-liner fallback.
fn expected(engine: &str) -> Error {
    Error::InvalidDbUrl(format!("Expected {} connection", engine))
}

#[tauri::command]
#[specta::specta]
pub async fn connect_project_db(
    project_path: String,
    conn_key: String,
    registry: State<'_, ConnectionRegistry>,
    app: tauri::AppHandle,
) -> Result<String, Error> {
    ensure_connection(&project_path, &conn_key, &registry, app).await
}

#[tauri::command]
#[specta::specta]
pub async fn get_connection_info(
    conn_id: String,
    registry: State<'_, ConnectionRegistry>,
) -> Result<ConnectionInfo, Error> {
    registry
        .get_info_by_str(&conn_id)
        .await
        .ok_or_else(|| Error::InvalidDbUrl(format!("Connection not found: {}", conn_id)))
}

#[tauri::command]
#[specta::specta]
pub async fn close_connection(
    project_path: String,
    conn_key: String,
    registry: State<'_, ConnectionRegistry>,
) -> Result<(), Error> {
    let id = ConnectionRegistry::get_id(&project_path, &conn_key);
    registry.close(&id).await;
    Ok(())
}

#[tauri::command]
#[specta::specta]
pub async fn close_project_connections(
    project_path: String,
    registry: State<'_, ConnectionRegistry>,
) -> Result<(), Error> {
    registry.close_project(&project_path).await;
    Ok(())
}

// ---------------------------------------------------------------------------
// Tree / schema listing commands
// ---------------------------------------------------------------------------

#[tauri::command]
#[specta::specta]
pub async fn get_sqlite_objects(
    project_path: String,
    conn_key: String,
    object_type: String,
    registry: State<'_, ConnectionRegistry>,
    app: tauri::AppHandle,
) -> Result<Vec<SQLiteObject>, Error> {
    let conn_id = ensure_connection(&project_path, &conn_key, &registry, app).await?;
    let pools = registry.pools().await;
    let pool = pools
        .get(&conn_id)
        .ok_or_else(|| Error::ConnectionNotFound(conn_id.clone()))?;
    match pool {
        ConnectionPool::Sqlite(p) => engine::sqlite::list_objects(p, &object_type).await,
        _ => Err(expected("SQLite")),
    }
}

#[tauri::command]
#[specta::specta]
pub async fn get_mongodb_collections(
    project_path: String,
    conn_key: String,
    registry: State<'_, ConnectionRegistry>,
    app: tauri::AppHandle,
) -> Result<Vec<MongoDBCollection>, Error> {
    let conn_id = ensure_connection(&project_path, &conn_key, &registry, app).await?;
    let pools = registry.pools().await;
    let pool = pools
        .get(&conn_id)
        .ok_or_else(|| Error::ConnectionNotFound(conn_id.clone()))?;
    match pool {
        ConnectionPool::Mongo(db) => engine::mongo::list_collections(db).await,
        _ => Err(expected("MongoDB")),
    }
}

#[tauri::command]
#[specta::specta]
pub async fn get_postgres_schemas(
    project_path: String,
    conn_key: String,
    registry: State<'_, ConnectionRegistry>,
    app: tauri::AppHandle,
) -> Result<Vec<PostgresSchema>, Error> {
    let conn_id = ensure_connection(&project_path, &conn_key, &registry, app).await?;
    let pools = registry.pools().await;
    let pool = pools
        .get(&conn_id)
        .ok_or_else(|| Error::ConnectionNotFound(conn_id.clone()))?;
    match pool {
        ConnectionPool::Postgres(p) => engine::postgres::list_schemas(p).await,
        _ => Err(expected("PostgreSQL")),
    }
}

#[tauri::command]
#[specta::specta]
pub async fn get_postgres_tables(
    project_path: String,
    conn_key: String,
    schema: String,
    registry: State<'_, ConnectionRegistry>,
    app: tauri::AppHandle,
) -> Result<Vec<PostgresTable>, Error> {
    let conn_id = ensure_connection(&project_path, &conn_key, &registry, app).await?;
    let pools = registry.pools().await;
    let pool = pools
        .get(&conn_id)
        .ok_or_else(|| Error::ConnectionNotFound(conn_id.clone()))?;
    match pool {
        ConnectionPool::Postgres(p) => engine::postgres::list_tables(p, &schema).await,
        _ => Err(expected("PostgreSQL")),
    }
}

// ---------------------------------------------------------------------------
// Browse commands
// ---------------------------------------------------------------------------

#[tauri::command]
#[specta::specta]
pub async fn query_sqlite_table(
    project_path: String,
    conn_key: String,
    table_name: String,
    options: BrowseOptions,
    registry: State<'_, ConnectionRegistry>,
    app: tauri::AppHandle,
) -> Result<QueryResult, Error> {
    let conn_id = ensure_connection(&project_path, &conn_key, &registry, app).await?;
    let pools = registry.pools().await;
    let pool = pools
        .get(&conn_id)
        .ok_or_else(|| Error::ConnectionNotFound(conn_id.clone()))?;
    match pool {
        ConnectionPool::Sqlite(p) => engine::sqlite::browse_table(p, &table_name, &options).await,
        _ => Err(expected("SQLite")),
    }
}

#[tauri::command]
#[specta::specta]
pub async fn query_postgres_table(
    project_path: String,
    conn_key: String,
    schema: String,
    table_name: String,
    options: BrowseOptions,
    registry: State<'_, ConnectionRegistry>,
    app: tauri::AppHandle,
) -> Result<QueryResult, Error> {
    let conn_id = ensure_connection(&project_path, &conn_key, &registry, app).await?;
    let pools = registry.pools().await;
    let pool = pools
        .get(&conn_id)
        .ok_or_else(|| Error::ConnectionNotFound(conn_id.clone()))?;
    match pool {
        ConnectionPool::Postgres(p) => {
            engine::postgres::browse_table(p, &schema, &table_name, &options).await
        }
        _ => Err(expected("PostgreSQL")),
    }
}

#[tauri::command]
#[specta::specta]
pub async fn query_mongodb_collection(
    project_path: String,
    conn_key: String,
    collection_name: String,
    options: BrowseOptions,
    registry: State<'_, ConnectionRegistry>,
    app: tauri::AppHandle,
) -> Result<QueryResult, Error> {
    let conn_id = ensure_connection(&project_path, &conn_key, &registry, app).await?;
    let pools = registry.pools().await;
    let pool = pools
        .get(&conn_id)
        .ok_or_else(|| Error::ConnectionNotFound(conn_id.clone()))?;
    match pool {
        ConnectionPool::Mongo(db) => {
            engine::mongo::browse_collection(db, &collection_name, &options).await
        }
        _ => Err(expected("MongoDB")),
    }
}

// ---------------------------------------------------------------------------
// Raw query execution commands
// ---------------------------------------------------------------------------

#[tauri::command]
#[specta::specta]
pub async fn execute_raw_sql(
    project_path: String,
    conn_key: String,
    query: String,
    registry: State<'_, ConnectionRegistry>,
    app: tauri::AppHandle,
) -> Result<QueryResult, Error> {
    let conn_id = ensure_connection(&project_path, &conn_key, &registry, app).await?;
    let pools = registry.pools().await;
    let pool = pools
        .get(&conn_id)
        .ok_or_else(|| Error::ConnectionNotFound(conn_id.clone()))?;
    match pool {
        ConnectionPool::Sqlite(p) => engine::sqlite::execute_raw(p, &query).await,
        ConnectionPool::Postgres(p) => engine::postgres::execute_raw(p, &query).await,
        ConnectionPool::Mongo(_) => Err(Error::InvalidDbUrl(
            "Use execute_raw_mongo for MongoDB queries".to_string(),
        )),
    }
}

#[tauri::command]
#[specta::specta]
pub async fn execute_raw_mongo(
    project_path: String,
    conn_key: String,
    collection: String,
    query_type: String,
    query: String,
    registry: State<'_, ConnectionRegistry>,
    app: tauri::AppHandle,
) -> Result<QueryResult, Error> {
    let conn_id = ensure_connection(&project_path, &conn_key, &registry, app).await?;
    let pools = registry.pools().await;
    let pool = pools
        .get(&conn_id)
        .ok_or_else(|| Error::ConnectionNotFound(conn_id.clone()))?;
    match pool {
        ConnectionPool::Mongo(db) => {
            engine::mongo::execute_raw(db, &collection, &query_type, &query).await
        }
        _ => Err(expected("MongoDB")),
    }
}
