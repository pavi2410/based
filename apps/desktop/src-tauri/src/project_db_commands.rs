use crate::connection_id::{ConnectionInfo, ConnectionRegistry};
use crate::connection_pool::ConnectionPool;
use crate::error::Error;
use crate::project_commands::read_project_config;
use crate::project_types::{ConnectionConfig, Engine};
use serde::{Deserialize, Serialize};
use sqlx::Row;
use std::collections::HashMap;
use tauri::State;

/// Build connection string from connection config and environment variables.
fn build_connection_string(
    conn_config: &ConnectionConfig,
    project_path: &str,
    env_vars: &HashMap<String, String>,
) -> Result<String, Error> {
    match conn_config.engine {
        Engine::Sqlite => {
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
            Ok(format!("sqlite:{}", absolute_path))
        }
        Engine::MongoDB => {
            let url_secret = conn_config.url.as_ref()
                .ok_or_else(|| Error::InvalidDbUrl("MongoDB connection missing 'url' field".to_string()))?;

            url_secret.resolve(env_vars)
                .map_err(|e| Error::InvalidDbUrl(format!("Failed to resolve MongoDB URL: {}", e)))
        }
        Engine::Postgres => {
            let host = conn_config.host.as_ref()
                .ok_or_else(|| Error::InvalidDbUrl("PostgreSQL connection missing 'host' field".to_string()))?;
            let port = conn_config.port.unwrap_or(5432);
            let database = conn_config.database.as_ref()
                .ok_or_else(|| Error::InvalidDbUrl("PostgreSQL connection missing 'database' field".to_string()))?;
            let username = conn_config.username.as_ref()
                .ok_or_else(|| Error::InvalidDbUrl("PostgreSQL connection missing 'username' field".to_string()))?;

            let password = if let Some(pass_secret) = &conn_config.password {
                pass_secret.resolve(env_vars)
                    .map_err(|e| Error::InvalidDbUrl(format!("Failed to resolve PostgreSQL password: {}", e)))?
            } else {
                String::new()
            };

            let ssl_mode = if conn_config.ssl.unwrap_or(false) { "require" } else { "disable" };
            Ok(format!("postgresql://{}:{}@{}:{}/{}?sslmode={}", username, password, host, port, database, ssl_mode))
        }
    }
}

/// Load environment variables from project's .env file.
fn load_project_env(project_path: &str) -> HashMap<String, String> {
    // load_env_file expects the project root path and joins .based/.env internally
    crate::variables::load_env_file(project_path)
        .unwrap_or_else(|_| HashMap::new())
}

/// Connect to a project database and return its connection ID.
/// If already connected, returns the existing connection ID.
#[tauri::command]
pub async fn connect_project_db(
    project_path: String,
    conn_key: String,
    registry: State<'_, ConnectionRegistry>,
    app: tauri::AppHandle,
) -> Result<String, Error> {
    // Check if connection already exists
    if registry.contains_by_key(&project_path, &conn_key).await {
        let id = ConnectionRegistry::get_id(&project_path, &conn_key);
        return Ok(id.to_string());
    }

    // Load project config
    let config = read_project_config(project_path.clone())
        .await
        .map_err(|e| Error::InvalidDbUrl(e))?;

    // Get connection config
    let conn_config = config
        .connection
        .get(&conn_key)
        .ok_or_else(|| Error::InvalidDbUrl(format!("Connection key not found: {}", conn_key)))?;

    // Load environment variables
    let env_vars = load_project_env(&project_path);

    // Build connection string
    let conn_string = build_connection_string(conn_config, &project_path, &env_vars)?;

    // Create connection
    let pool = ConnectionPool::connect(&conn_string, &app).await?;

    // Register in the registry
    let id = registry.register(
        project_path,
        conn_key,
        conn_config.engine.clone(),
        conn_config.label.clone(),
        pool,
    ).await;

    Ok(id.to_string())
}

/// Get connection info by ID.
#[tauri::command]
pub async fn get_connection_info(
    conn_id: String,
    registry: State<'_, ConnectionRegistry>,
) -> Result<ConnectionInfo, Error> {
    registry
        .get_info_by_str(&conn_id)
        .await
        .ok_or_else(|| Error::InvalidDbUrl(format!("Connection not found: {}", conn_id)))
}

// ============================================================================
// Database Object Query Commands
// ============================================================================

#[derive(Debug, Serialize, Deserialize)]
pub struct SQLiteObject {
    name: String,
}

#[tauri::command]
pub async fn get_sqlite_objects(
    project_path: String,
    conn_key: String,
    object_type: String,
    registry: State<'_, ConnectionRegistry>,
    app: tauri::AppHandle,
) -> Result<Vec<SQLiteObject>, Error> {
    // Ensure connection exists
    let conn_id = ensure_connection(&project_path, &conn_key, &registry, app).await?;

    // Get pool and execute query
    let pools = registry.pools().await;
    let pool = pools
        .get(&conn_id)
        .ok_or_else(|| Error::ConnectionNotFound(conn_id.clone()))?;

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
    registry: State<'_, ConnectionRegistry>,
    app: tauri::AppHandle,
) -> Result<Vec<MongoDBCollection>, Error> {
    // Ensure connection exists
    let conn_id = ensure_connection(&project_path, &conn_key, &registry, app).await?;

    // Get pool and execute query
    let pools = registry.pools().await;
    let pool = pools
        .get(&conn_id)
        .ok_or_else(|| Error::ConnectionNotFound(conn_id.clone()))?;

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
    registry: State<'_, ConnectionRegistry>,
    app: tauri::AppHandle,
) -> Result<Vec<PostgresSchema>, Error> {
    // Ensure connection exists
    let conn_id = ensure_connection(&project_path, &conn_key, &registry, app).await?;

    // Get pool and execute query
    let pools = registry.pools().await;
    let pool = pools
        .get(&conn_id)
        .ok_or_else(|| Error::ConnectionNotFound(conn_id.clone()))?;

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
    registry: State<'_, ConnectionRegistry>,
    app: tauri::AppHandle,
) -> Result<Vec<PostgresTable>, Error> {
    // Ensure connection exists
    let conn_id = ensure_connection(&project_path, &conn_key, &registry, app).await?;

    // Get pool and execute query
    let pools = registry.pools().await;
    let pool = pools
        .get(&conn_id)
        .ok_or_else(|| Error::ConnectionNotFound(conn_id.clone()))?;

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
    registry: State<'_, ConnectionRegistry>,
) -> Result<(), Error> {
    registry.close_project(&project_path).await;
    Ok(())
}

// ============================================================================
// Helper Functions
// ============================================================================

/// Ensure a connection exists, creating it if necessary.
/// Returns the connection ID.
async fn ensure_connection(
    project_path: &str,
    conn_key: &str,
    registry: &State<'_, ConnectionRegistry>,
    app: tauri::AppHandle,
) -> Result<String, Error> {
    // Check if connection already exists
    if registry.contains_by_key(project_path, conn_key).await {
        let id = ConnectionRegistry::get_id(project_path, conn_key);
        return Ok(id.to_string());
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

    // Load environment variables
    let env_vars = load_project_env(project_path);

    // Build connection string
    let conn_string = build_connection_string(conn_config, project_path, &env_vars)?;

    // Create connection
    let pool = ConnectionPool::connect(&conn_string, &app).await?;

    // Register in the registry
    let id = registry.register(
        project_path.to_string(),
        conn_key.to_string(),
        conn_config.engine.clone(),
        conn_config.label.clone(),
        pool,
    ).await;

    Ok(id.to_string())
}
