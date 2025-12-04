use crate::connection_pool::ConnectionPool;
use crate::error::Error;
use crate::project_commands::read_project_config;
use crate::DbInstances;
use serde::{Deserialize, Serialize};
use sqlx::Row;
use std::collections::HashMap;
use tauri::State;

/// Generate a project-aware connection key
/// Format: {projectPath}::{connKey}
fn get_connection_key(project_path: &str, conn_key: &str) -> String {
    format!("{}::{}", project_path, conn_key)
}

/// Ensure a project database connection exists in the pool
async fn ensure_project_connection(
    project_path: &str,
    conn_key: &str,
    db_instances: &State<'_, DbInstances>,
    app: tauri::AppHandle,
) -> Result<(), Error> {
    let pool_key = get_connection_key(project_path, conn_key);

    // Check if connection already exists
    {
        let instances = db_instances.0.read().await;
        if instances.contains_key(&pool_key) {
            return Ok(());
        }
    }

    // Load project config
    let config = read_project_config(project_path.to_string())
        .await
        .map_err(|e| Error::InvalidDbUrl(e))?;

    // Get connection config
    let conn_config = config
        .connection
        .get(conn_key)
        .ok_or_else(|| Error::InvalidDbUrl(format!("Connection key not found: {}", conn_key)))?;

    // Load environment file from .based/.env
    let env_file_path = std::path::Path::new(project_path).join(".based/.env");
    let env_vars = if env_file_path.exists() {
        crate::variables::load_env_file(&env_file_path.to_string_lossy().to_string())
            .unwrap_or_else(|_| HashMap::new())
    } else {
        HashMap::new()
    };

    // Resolve connection string based on engine type
    use crate::project_types::Engine;
    let resolved_conn_string = match conn_config.engine {
        Engine::Sqlite => {
            // SQLite
            let file = conn_config.file.as_ref()
                .ok_or_else(|| Error::InvalidDbUrl("SQLite connection missing 'file' field".to_string()))?;

            // Make path absolute if relative
            let absolute_path = if std::path::Path::new(&file).is_relative() {
                std::path::Path::new(project_path)
                    .join(&file)
                    .to_string_lossy()
                    .to_string()
            } else {
                file.clone()
            };
            format!("sqlite:{}", absolute_path)
        }
        Engine::MongoDB => {
            // MongoDB
            let url_secret = conn_config.url.as_ref()
                .ok_or_else(|| Error::InvalidDbUrl("MongoDB connection missing 'url' field".to_string()))?;

            url_secret.resolve(&env_vars)
                .map_err(|e| Error::InvalidDbUrl(format!("Failed to resolve MongoDB URL: {}", e)))?
        }
        Engine::Postgres => {
            // PostgreSQL - construct URL from parts or use password secret
            let host = conn_config.host.as_ref()
                .ok_or_else(|| Error::InvalidDbUrl("PostgreSQL connection missing 'host' field".to_string()))?;
            let port = conn_config.port.unwrap_or(5432);
            let database = conn_config.database.as_ref()
                .ok_or_else(|| Error::InvalidDbUrl("PostgreSQL connection missing 'database' field".to_string()))?;
            let username = conn_config.username.as_ref()
                .ok_or_else(|| Error::InvalidDbUrl("PostgreSQL connection missing 'username' field".to_string()))?;

            let password = if let Some(pass_secret) = &conn_config.password {
                pass_secret.resolve(&env_vars)
                    .map_err(|e| Error::InvalidDbUrl(format!("Failed to resolve PostgreSQL password: {}", e)))?
            } else {
                String::new()
            };

            let ssl_mode = if conn_config.ssl.unwrap_or(false) { "require" } else { "disable" };
            format!("postgresql://{}:{}@{}:{}/{}?sslmode={}", username, password, host, port, database, ssl_mode)
        }
    };

    // Create connection
    let pool = ConnectionPool::connect(&resolved_conn_string, &app).await?;

    // Store in pool
    let mut instances = db_instances.0.write().await;
    instances.insert(pool_key, pool);

    Ok(())
}

#[derive(Debug, Serialize, Deserialize)]
pub struct SQLiteObject {
    name: String,
}

#[tauri::command]
pub async fn get_sqlite_objects(
    project_path: String,
    conn_key: String,
    object_type: String,
    db_instances: State<'_, DbInstances>,
    app: tauri::AppHandle,
) -> Result<Vec<SQLiteObject>, Error> {
    // Ensure connection exists
    ensure_project_connection(&project_path, &conn_key, &db_instances, app).await?;

    let pool_key = get_connection_key(&project_path, &conn_key);

    let instances = db_instances.0.read().await;
    let pool = instances
        .get(&pool_key)
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
    conn_key: String,
    db_instances: State<'_, DbInstances>,
    app: tauri::AppHandle,
) -> Result<Vec<MongoDBCollection>, Error> {
    // Ensure connection exists
    ensure_project_connection(&project_path, &conn_key, &db_instances, app).await?;

    let pool_key = get_connection_key(&project_path, &conn_key);

    let instances = db_instances.0.read().await;
    let pool = instances
        .get(&pool_key)
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

#[derive(Debug, Serialize, Deserialize)]
pub struct PostgresSchema {
    name: String,
}

#[tauri::command]
pub async fn get_postgres_schemas(
    project_path: String,
    conn_key: String,
    db_instances: State<'_, DbInstances>,
    app: tauri::AppHandle,
) -> Result<Vec<PostgresSchema>, Error> {
    // Ensure connection exists
    ensure_project_connection(&project_path, &conn_key, &db_instances, app).await?;

    let pool_key = get_connection_key(&project_path, &conn_key);

    let instances = db_instances.0.read().await;
    let pool = instances
        .get(&pool_key)
        .ok_or_else(|| Error::InvalidDbUrl("Connection not found".to_string()))?;

    match pool {
        ConnectionPool::Postgres(pool) => {
            let query = "SELECT schema_name as name FROM information_schema.schemata
                         WHERE schema_name NOT IN ('pg_catalog', 'information_schema', 'pg_toast')
                         ORDER BY schema_name";

            let rows = sqlx::query(query).fetch_all(pool).await?;

            let schemas: Vec<PostgresSchema> = rows
                .iter()
                .map(|row| PostgresSchema {
                    name: row.get(0),
                })
                .collect();

            Ok(schemas)
        }
        _ => Err(Error::InvalidDbUrl(
            "Expected PostgreSQL connection".to_string(),
        )),
    }
}

#[derive(Debug, Serialize, Deserialize)]
pub struct PostgresTable {
    name: String,
    schema: String,
}

#[tauri::command]
pub async fn get_postgres_tables(
    project_path: String,
    conn_key: String,
    schema: String,
    db_instances: State<'_, DbInstances>,
    app: tauri::AppHandle,
) -> Result<Vec<PostgresTable>, Error> {
    // Ensure connection exists
    ensure_project_connection(&project_path, &conn_key, &db_instances, app).await?;

    let pool_key = get_connection_key(&project_path, &conn_key);

    let instances = db_instances.0.read().await;
    let pool = instances
        .get(&pool_key)
        .ok_or_else(|| Error::InvalidDbUrl("Connection not found".to_string()))?;

    match pool {
        ConnectionPool::Postgres(pool) => {
            let query = "SELECT table_name as name, table_schema as schema
                         FROM information_schema.tables
                         WHERE table_schema = $1 AND table_type = 'BASE TABLE'
                         ORDER BY table_name";

            let rows = sqlx::query(query)
                .bind(&schema)
                .fetch_all(pool)
                .await?;

            let tables: Vec<PostgresTable> = rows
                .iter()
                .map(|row| PostgresTable {
                    name: row.get(0),
                    schema: row.get(1),
                })
                .collect();

            Ok(tables)
        }
        _ => Err(Error::InvalidDbUrl(
            "Expected PostgreSQL connection".to_string(),
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
