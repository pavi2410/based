use crate::connection_pool::ConnectionPool;
use crate::error::Error;
use crate::project_commands::read_project_config;
use crate::variables::{resolve_variables, VariableError};
use crate::DbInstances;
use serde::{Deserialize, Serialize};
use sqlx::Row;
use std::collections::HashMap;
use tauri::State;

/// Generate a project-aware connection key
/// Format: {projectPath}::{dbKey}::{env}
fn get_connection_key(project_path: &str, db_key: &str, env: &str) -> String {
    format!("{}::{}::{}", project_path, db_key, env)
}

/// Ensure a project database connection exists in the pool
async fn ensure_project_connection(
    project_path: &str,
    db_key: &str,
    environment: &str,
    db_instances: &State<'_, DbInstances>,
    app: tauri::AppHandle,
) -> Result<(), Error> {
    let conn_key = get_connection_key(project_path, db_key, environment);

    // Check if connection already exists
    {
        let instances = db_instances.0.read().await;
        if instances.contains_key(&conn_key) {
            return Ok(());
        }
    }

    // Load project config
    let config = read_project_config(project_path.to_string())
        .await
        .map_err(|e| Error::InvalidDbUrl(e))?;

    // Get database config
    let db_config = config
        .databases
        .get(db_key)
        .ok_or_else(|| Error::InvalidDbUrl(format!("Database key not found: {}", db_key)))?;

    // Load environment file - for now, just use empty map since we don't have env-specific file loading
    // TODO: Implement environment-specific .env file loading
    let env_vars: HashMap<String, String> = HashMap::new();

    // Resolve connection string based on database type
    let resolved_conn_string = match &db_config.connection {
        connection if connection.path.is_some() => {
            // SQLite
            let path = connection.path.as_ref().unwrap();
            let resolved_path = resolve_variables(path, &env_vars)
                .map_err(|e| Error::InvalidDbUrl(format!("Variable resolution failed: {:?}", e)))?;
            // Make path absolute if relative
            let absolute_path = if std::path::Path::new(&resolved_path).is_relative() {
                std::path::Path::new(project_path)
                    .join(&resolved_path)
                    .to_string_lossy()
                    .to_string()
            } else {
                resolved_path
            };
            format!("sqlite:{}", absolute_path)
        }
        connection if connection.url.is_some() => {
            // MongoDB or PostgreSQL (both use URL)
            let url = connection.url.as_ref().unwrap();
            resolve_variables(url, &env_vars)
                .map_err(|e| Error::InvalidDbUrl(format!("Variable resolution failed: {:?}", e)))?
        }
        _ => {
            return Err(Error::InvalidDbUrl(
                "No valid connection config found".to_string(),
            ));
        }
    };

    // Create connection
    let pool = ConnectionPool::connect(&resolved_conn_string, &app).await?;

    // Store in pool
    let mut instances = db_instances.0.write().await;
    instances.insert(conn_key, pool);

    Ok(())
}

#[derive(Debug, Serialize, Deserialize)]
pub struct SQLiteObject {
    name: String,
}

#[tauri::command]
pub async fn get_sqlite_objects(
    project_path: String,
    db_key: String,
    environment: String,
    object_type: String,
    db_instances: State<'_, DbInstances>,
    app: tauri::AppHandle,
) -> Result<Vec<SQLiteObject>, Error> {
    // Ensure connection exists
    ensure_project_connection(&project_path, &db_key, &environment, &db_instances, app).await?;

    let conn_key = get_connection_key(&project_path, &db_key, &environment);

    let instances = db_instances.0.read().await;
    let pool = instances
        .get(&conn_key)
        .ok_or_else(|| Error::InvalidDbUrl("Connection not found".to_string()))?;

    match pool {
        ConnectionPool::Sqlite(pool) => {
            let query = format!(
                "SELECT name FROM sqlite_schema WHERE type = '{}' ORDER BY name",
                object_type
            );

            let rows = sqlx::query(&query).fetch_all(pool).await?;

            let objects: Vec<SQLiteObject> = rows
                .iter()
                .map(|row| SQLiteObject {
                    name: row.get(0),
                })
                .collect();

            Ok(objects)
        }
        _ => Err(Error::InvalidDbUrl(
            "Expected SQLite connection".to_string(),
        )),
    }
}

#[derive(Debug, Serialize, Deserialize)]
pub struct MongoDBCollection {
    name: String,
}

#[tauri::command]
pub async fn get_mongodb_collections(
    project_path: String,
    db_key: String,
    environment: String,
    db_instances: State<'_, DbInstances>,
    app: tauri::AppHandle,
) -> Result<Vec<MongoDBCollection>, Error> {
    // Ensure connection exists
    ensure_project_connection(&project_path, &db_key, &environment, &db_instances, app).await?;

    let conn_key = get_connection_key(&project_path, &db_key, &environment);

    let instances = db_instances.0.read().await;
    let pool = instances
        .get(&conn_key)
        .ok_or_else(|| Error::InvalidDbUrl("Connection not found".to_string()))?;

    match pool {
        ConnectionPool::Mongo(db) => {
            let collection_names = db.list_collection_names(None).await?;

            let collections: Vec<MongoDBCollection> = collection_names
                .into_iter()
                .map(|name| MongoDBCollection { name })
                .collect();

            Ok(collections)
        }
        _ => Err(Error::InvalidDbUrl(
            "Expected MongoDB connection".to_string(),
        )),
    }
}

#[tauri::command]
pub async fn close_project_connections(
    project_path: String,
    db_instances: State<'_, DbInstances>,
) -> Result<(), Error> {
    let mut instances = db_instances.0.write().await;

    // Find all keys that start with this project path
    let keys_to_remove: Vec<String> = instances
        .keys()
        .filter(|k| k.starts_with(&format!("{}::", project_path)))
        .cloned()
        .collect();

    // Close and remove connections
    for key in keys_to_remove {
        if let Some(pool) = instances.remove(&key) {
            pool.close().await;
        }
    }

    Ok(())
}
