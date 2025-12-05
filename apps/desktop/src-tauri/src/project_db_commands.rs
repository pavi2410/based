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

/// Close a specific connection by project path and connection key.
#[tauri::command]
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
pub async fn close_project_connections(
    project_path: String,
    registry: State<'_, ConnectionRegistry>,
) -> Result<(), Error> {
    registry.close_project(&project_path).await;
    Ok(())
}

// ============================================================================
// Data Query Commands
// ============================================================================

/// Result of a data query - rows with column info
#[derive(Debug, Serialize, Deserialize)]
pub struct QueryResult {
    pub columns: Vec<ColumnInfo>,
    pub rows: Vec<Vec<serde_json::Value>>,
    pub total_count: Option<i64>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct ColumnInfo {
    pub name: String,
    pub data_type: String,
}

/// Query data from a SQLite table
#[tauri::command]
pub async fn query_sqlite_table(
    project_path: String,
    conn_key: String,
    table_name: String,
    limit: Option<i64>,
    offset: Option<i64>,
    order_by_column: Option<String>,
    order_by_direction: Option<String>,
    registry: State<'_, ConnectionRegistry>,
    app: tauri::AppHandle,
) -> Result<QueryResult, Error> {
    let conn_id = ensure_connection(&project_path, &conn_key, &registry, app).await?;

    let pools = registry.pools().await;
    let pool = pools
        .get(&conn_id)
        .ok_or_else(|| Error::ConnectionNotFound(conn_id.clone()))?;

    match pool {
        ConnectionPool::Sqlite(pool) => {
            // Get column info
            let pragma_query = format!("PRAGMA table_info('{}')", table_name);
            let column_rows = sqlx::query(&pragma_query).fetch_all(pool).await?;
            
            let columns: Vec<ColumnInfo> = column_rows
                .iter()
                .map(|row| ColumnInfo {
                    name: row.get::<String, _>("name"),
                    data_type: row.get::<String, _>("type"),
                })
                .collect();

            // Get total count
            let count_query = format!("SELECT COUNT(*) as cnt FROM \"{}\"", table_name);
            let count_row = sqlx::query(&count_query).fetch_one(pool).await?;
            let total_count: i64 = count_row.get("cnt");

            // Query data with pagination and sorting
            let limit_val = limit.unwrap_or(100);
            let offset_val = offset.unwrap_or(0);
            let order_clause = match (&order_by_column, &order_by_direction) {
                (Some(col), Some(dir)) => {
                    let direction = if dir.to_lowercase() == "desc" { "DESC" } else { "ASC" };
                    format!(" ORDER BY \"{}\" {}", col, direction)
                }
                _ => String::new(),
            };
            let data_query = format!(
                "SELECT * FROM \"{}\"{} LIMIT {} OFFSET {}",
                table_name, order_clause, limit_val, offset_val
            );
            
            let data_rows = sqlx::query(&data_query).fetch_all(pool).await?;
            
            let rows: Vec<Vec<serde_json::Value>> = data_rows
                .iter()
                .map(|row| {
                    columns
                        .iter()
                        .map(|col| sqlite_value_to_json(row, &col.name))
                        .collect()
                })
                .collect();

            Ok(QueryResult {
                columns,
                rows,
                total_count: Some(total_count),
            })
        }
        _ => Err(Error::InvalidDbUrl("Expected SQLite connection".to_string())),
    }
}

/// Convert SQLite row value to JSON
fn sqlite_value_to_json(row: &sqlx::sqlite::SqliteRow, column: &str) -> serde_json::Value {
    use sqlx::Row;
    use sqlx::ValueRef;
    
    let value_ref = row.try_get_raw(column);
    match value_ref {
        Ok(val) if val.is_null() => serde_json::Value::Null,
        Ok(_) => {
            // Try different types
            if let Ok(v) = row.try_get::<i64, _>(column) {
                serde_json::Value::Number(v.into())
            } else if let Ok(v) = row.try_get::<f64, _>(column) {
                serde_json::Number::from_f64(v)
                    .map(serde_json::Value::Number)
                    .unwrap_or(serde_json::Value::Null)
            } else if let Ok(v) = row.try_get::<String, _>(column) {
                serde_json::Value::String(v)
            } else if let Ok(v) = row.try_get::<bool, _>(column) {
                serde_json::Value::Bool(v)
            } else if let Ok(v) = row.try_get::<Vec<u8>, _>(column) {
                // Binary data - show size
                serde_json::Value::String(format!("[BLOB: {} bytes]", v.len()))
            } else {
                serde_json::Value::Null
            }
        }
        Err(_) => serde_json::Value::Null,
    }
}

/// Query data from a PostgreSQL table
#[tauri::command]
pub async fn query_postgres_table(
    project_path: String,
    conn_key: String,
    schema: String,
    table_name: String,
    limit: Option<i64>,
    offset: Option<i64>,
    order_by_column: Option<String>,
    order_by_direction: Option<String>,
    registry: State<'_, ConnectionRegistry>,
    app: tauri::AppHandle,
) -> Result<QueryResult, Error> {
    let conn_id = ensure_connection(&project_path, &conn_key, &registry, app).await?;

    let pools = registry.pools().await;
    let pool = pools
        .get(&conn_id)
        .ok_or_else(|| Error::ConnectionNotFound(conn_id.clone()))?;

    match pool {
        ConnectionPool::Postgres(pool) => {
            // Get column info
            let column_query = r#"
                SELECT column_name, data_type 
                FROM information_schema.columns 
                WHERE table_schema = $1 AND table_name = $2
                ORDER BY ordinal_position
            "#;
            let column_rows = sqlx::query(column_query)
                .bind(&schema)
                .bind(&table_name)
                .fetch_all(pool)
                .await?;
            
            let columns: Vec<ColumnInfo> = column_rows
                .iter()
                .map(|row| ColumnInfo {
                    name: row.get("column_name"),
                    data_type: row.get("data_type"),
                })
                .collect();

            // Get total count
            let count_query = format!("SELECT COUNT(*) as cnt FROM \"{}\".\"{}\"", schema, table_name);
            let count_row = sqlx::query(&count_query).fetch_one(pool).await?;
            let total_count: i64 = count_row.get("cnt");

            // Query data with pagination and sorting
            let limit_val = limit.unwrap_or(100);
            let offset_val = offset.unwrap_or(0);
            let order_clause = match (&order_by_column, &order_by_direction) {
                (Some(col), Some(dir)) => {
                    let direction = if dir.to_lowercase() == "desc" { "DESC" } else { "ASC" };
                    format!(" ORDER BY \"{}\" {}", col, direction)
                }
                _ => String::new(),
            };
            let data_query = format!(
                "SELECT * FROM \"{}\".\"{}\"{} LIMIT {} OFFSET {}",
                schema, table_name, order_clause, limit_val, offset_val
            );
            
            let data_rows = sqlx::query(&data_query).fetch_all(pool).await?;
            
            let rows: Vec<Vec<serde_json::Value>> = data_rows
                .iter()
                .map(|row| {
                    columns
                        .iter()
                        .map(|col| postgres_value_to_json(row, &col.name))
                        .collect()
                })
                .collect();

            Ok(QueryResult {
                columns,
                rows,
                total_count: Some(total_count),
            })
        }
        _ => Err(Error::InvalidDbUrl("Expected PostgreSQL connection".to_string())),
    }
}

/// Convert PostgreSQL row value to JSON
fn postgres_value_to_json(row: &sqlx::postgres::PgRow, column: &str) -> serde_json::Value {
    use sqlx::Row;
    use sqlx::ValueRef;
    
    let value_ref = row.try_get_raw(column);
    match value_ref {
        Ok(val) if val.is_null() => serde_json::Value::Null,
        Ok(_) => {
            // Try different types
            if let Ok(v) = row.try_get::<i64, _>(column) {
                serde_json::Value::Number(v.into())
            } else if let Ok(v) = row.try_get::<i32, _>(column) {
                serde_json::Value::Number(v.into())
            } else if let Ok(v) = row.try_get::<f64, _>(column) {
                serde_json::Number::from_f64(v)
                    .map(serde_json::Value::Number)
                    .unwrap_or(serde_json::Value::Null)
            } else if let Ok(v) = row.try_get::<String, _>(column) {
                serde_json::Value::String(v)
            } else if let Ok(v) = row.try_get::<bool, _>(column) {
                serde_json::Value::Bool(v)
            } else if let Ok(v) = row.try_get::<chrono::NaiveDateTime, _>(column) {
                serde_json::Value::String(v.to_string())
            } else if let Ok(v) = row.try_get::<chrono::DateTime<chrono::Utc>, _>(column) {
                serde_json::Value::String(v.to_rfc3339())
            } else if let Ok(v) = row.try_get::<chrono::NaiveDate, _>(column) {
                serde_json::Value::String(v.to_string())
            } else if let Ok(v) = row.try_get::<serde_json::Value, _>(column) {
                v
            } else {
                // Fallback - try to get as string
                row.try_get::<String, _>(column)
                    .map(serde_json::Value::String)
                    .unwrap_or(serde_json::Value::Null)
            }
        }
        Err(_) => serde_json::Value::Null,
    }
}

/// Query data from a MongoDB collection
#[tauri::command]
pub async fn query_mongodb_collection(
    project_path: String,
    conn_key: String,
    collection_name: String,
    limit: Option<i64>,
    offset: Option<i64>,
    order_by_column: Option<String>,
    order_by_direction: Option<String>,
    registry: State<'_, ConnectionRegistry>,
    app: tauri::AppHandle,
) -> Result<QueryResult, Error> {
    use mongodb::bson::doc;
    use futures::TryStreamExt;

    let conn_id = ensure_connection(&project_path, &conn_key, &registry, app).await?;

    let pools = registry.pools().await;
    let pool = pools
        .get(&conn_id)
        .ok_or_else(|| Error::ConnectionNotFound(conn_id.clone()))?;

    match pool {
        ConnectionPool::Mongo(db) => {
            let collection = db.collection::<mongodb::bson::Document>(&collection_name);
            
            // Get total count
            let total_count = collection.count_documents(doc! {}, None).await? as i64;

            // Query data with pagination
            let limit_val = limit.unwrap_or(100);
            let offset_val = offset.unwrap_or(0);
            
            let sort_doc = match (&order_by_column, &order_by_direction) {
                (Some(col), Some(dir)) => {
                    let direction = if dir.to_lowercase() == "desc" { -1 } else { 1 };
                    Some(doc! { col.as_str(): direction })
                }
                _ => None,
            };
            let find_options = mongodb::options::FindOptions::builder()
                .limit(limit_val)
                .skip(offset_val as u64)
                .sort(sort_doc)
                .build();
            
            let mut cursor = collection.find(doc! {}, find_options).await?;
            
            let mut documents: Vec<mongodb::bson::Document> = Vec::new();
            while let Some(doc) = cursor.try_next().await? {
                documents.push(doc);
            }

            // Extract columns from first document (MongoDB is schemaless)
            let columns: Vec<ColumnInfo> = if let Some(first_doc) = documents.first() {
                first_doc
                    .keys()
                    .map(|key| ColumnInfo {
                        name: key.clone(),
                        data_type: "mixed".to_string(),
                    })
                    .collect()
            } else {
                vec![]
            };

            // Convert documents to rows
            let rows: Vec<Vec<serde_json::Value>> = documents
                .iter()
                .map(|doc| {
                    columns
                        .iter()
                        .map(|col| {
                            doc.get(&col.name)
                                .map(|v| bson_to_json(v))
                                .unwrap_or(serde_json::Value::Null)
                        })
                        .collect()
                })
                .collect();

            Ok(QueryResult {
                columns,
                rows,
                total_count: Some(total_count),
            })
        }
        _ => Err(Error::InvalidDbUrl("Expected MongoDB connection".to_string())),
    }
}

/// Convert BSON value to JSON
fn bson_to_json(bson: &mongodb::bson::Bson) -> serde_json::Value {
    use mongodb::bson::Bson;
    
    match bson {
        Bson::Null => serde_json::Value::Null,
        Bson::Boolean(b) => serde_json::Value::Bool(*b),
        Bson::Int32(i) => serde_json::Value::Number((*i).into()),
        Bson::Int64(i) => serde_json::Value::Number((*i).into()),
        Bson::Double(f) => serde_json::Number::from_f64(*f)
            .map(serde_json::Value::Number)
            .unwrap_or(serde_json::Value::Null),
        Bson::String(s) => serde_json::Value::String(s.clone()),
        Bson::ObjectId(oid) => serde_json::Value::String(oid.to_hex()),
        Bson::DateTime(dt) => serde_json::Value::String(dt.to_string()),
        Bson::Array(arr) => serde_json::Value::Array(
            arr.iter().map(bson_to_json).collect()
        ),
        Bson::Document(doc) => {
            let map: serde_json::Map<String, serde_json::Value> = doc
                .iter()
                .map(|(k, v)| (k.clone(), bson_to_json(v)))
                .collect();
            serde_json::Value::Object(map)
        }
        Bson::Binary(bin) => serde_json::Value::String(format!("[Binary: {} bytes]", bin.bytes.len())),
        Bson::Timestamp(ts) => serde_json::Value::String(format!("Timestamp({}, {})", ts.time, ts.increment)),
        Bson::RegularExpression(regex) => serde_json::Value::String(format!("/{}/{}", regex.pattern, regex.options)),
        _ => serde_json::Value::String(bson.to_string()),
    }
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
