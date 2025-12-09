use crate::connection_id::{ConnectionInfo, ConnectionRegistry};
use crate::connection_pool::ConnectionPool;
use crate::error::Error;
use crate::project_commands::read_project_config;
use crate::project_types::{ConnectionConfig, Engine};
use serde::{Deserialize, Serialize};
use sqlx::{Column, Row};
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
            // Support URL-based connection (like MongoDB)
            if let Some(url_secret) = &conn_config.url {
                return url_secret.resolve(env_vars)
                    .map_err(|e| Error::InvalidDbUrl(format!("Failed to resolve PostgreSQL URL: {}", e)));
            }

            // Fall back to individual fields
            let host = conn_config.host.as_ref()
                .ok_or_else(|| Error::InvalidDbUrl("PostgreSQL connection missing 'host' field (or use 'url' for connection string)".to_string()))?;
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

/// Filter parameter from frontend
#[derive(Debug, Serialize, Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct FilterParam {
    pub column_id: String,
    #[serde(rename = "type")]
    pub column_type: String,
    pub operator: String,
    pub values: Vec<serde_json::Value>,
}

/// Build SQL WHERE clause from filters
fn build_sql_where_clause(filters: &[FilterParam]) -> String {
    if filters.is_empty() {
        return String::new();
    }

    let conditions: Vec<String> = filters
        .iter()
        .filter_map(|f| build_sql_condition(f))
        .collect();

    if conditions.is_empty() {
        return String::new();
    }

    format!(" WHERE {}", conditions.join(" AND "))
}

/// Build a single SQL condition from a filter
fn build_sql_condition(filter: &FilterParam) -> Option<String> {
    let col = format!(r#""{}""#, filter.column_id);
    
    match filter.operator.as_str() {
        // Text operators
        "contains" => {
            let val = filter.values.first()?.as_str()?;
            Some(format!("{} LIKE '%{}%'", col, escape_sql_like(val)))
        }
        "does not contain" => {
            let val = filter.values.first()?.as_str()?;
            Some(format!("{} NOT LIKE '%{}%'", col, escape_sql_like(val)))
        }
        // Number/Date operators
        "is" => {
            let val = &filter.values.first()?;
            Some(format!("{} = {}", col, sql_value(val)))
        }
        "is not" => {
            let val = &filter.values.first()?;
            Some(format!("{} != {}", col, sql_value(val)))
        }
        "is less than" | "is before" => {
            let val = &filter.values.first()?;
            Some(format!("{} < {}", col, sql_value(val)))
        }
        "is less than or equal to" | "is on or before" => {
            let val = &filter.values.first()?;
            Some(format!("{} <= {}", col, sql_value(val)))
        }
        "is greater than" | "is after" => {
            let val = &filter.values.first()?;
            Some(format!("{} > {}", col, sql_value(val)))
        }
        "is greater than or equal to" | "is on or after" => {
            let val = &filter.values.first()?;
            Some(format!("{} >= {}", col, sql_value(val)))
        }
        "is between" => {
            if filter.values.len() >= 2 {
                let v1 = sql_value(&filter.values[0]);
                let v2 = sql_value(&filter.values[1]);
                Some(format!("{} BETWEEN {} AND {}", col, v1, v2))
            } else {
                None
            }
        }
        "is not between" => {
            if filter.values.len() >= 2 {
                let v1 = sql_value(&filter.values[0]);
                let v2 = sql_value(&filter.values[1]);
                Some(format!("{} NOT BETWEEN {} AND {}", col, v1, v2))
            } else {
                None
            }
        }
        // Option operators
        "is any of" => {
            let vals: Vec<String> = filter.values.iter().map(sql_value).collect();
            if vals.is_empty() {
                None
            } else {
                Some(format!("{} IN ({})", col, vals.join(", ")))
            }
        }
        "is none of" => {
            let vals: Vec<String> = filter.values.iter().map(sql_value).collect();
            if vals.is_empty() {
                None
            } else {
                Some(format!("{} NOT IN ({})", col, vals.join(", ")))
            }
        }
        _ => None,
    }
}

/// Escape special characters for SQL LIKE
fn escape_sql_like(s: &str) -> String {
    s.replace('\\', "\\\\")
        .replace('%', "\\%")
        .replace('_', "\\_")
        .replace('\'', "''")
}

/// Convert JSON value to SQL literal
fn sql_value(val: &serde_json::Value) -> String {
    match val {
        serde_json::Value::Null => "NULL".to_string(),
        serde_json::Value::Bool(b) => if *b { "1" } else { "0" }.to_string(),
        serde_json::Value::Number(n) => n.to_string(),
        serde_json::Value::String(s) => format!("'{}'", s.replace('\'', "''")),
        _ => "NULL".to_string(),
    }
}

/// Build MongoDB filter document from filters
fn build_mongodb_filter(filters: &[FilterParam]) -> mongodb::bson::Document {
    use mongodb::bson::{doc, Bson, Regex};

    if filters.is_empty() {
        return doc! {};
    }

    let conditions: Vec<mongodb::bson::Document> = filters
        .iter()
        .filter_map(|f| build_mongodb_condition(f))
        .collect();

    if conditions.is_empty() {
        return doc! {};
    }

    doc! { "$and": conditions }
}

/// Build a single MongoDB condition from a filter
fn build_mongodb_condition(filter: &FilterParam) -> Option<mongodb::bson::Document> {
    use mongodb::bson::{doc, Bson, Regex};

    let col = &filter.column_id;
    
    match filter.operator.as_str() {
        // Text operators
        "contains" => {
            let val = filter.values.first()?.as_str()?;
            let regex = Regex { pattern: regex::escape(val), options: "i".to_string() };
            Some(doc! { col: { "$regex": regex } })
        }
        "does not contain" => {
            let val = filter.values.first()?.as_str()?;
            let regex = Regex { pattern: regex::escape(val), options: "i".to_string() };
            Some(doc! { col: { "$not": { "$regex": regex } } })
        }
        // Equality operators
        "is" => {
            let val = json_to_bson(filter.values.first()?);
            Some(doc! { col: { "$eq": val } })
        }
        "is not" => {
            let val = json_to_bson(filter.values.first()?);
            Some(doc! { col: { "$ne": val } })
        }
        // Comparison operators
        "is less than" | "is before" => {
            let val = json_to_bson(filter.values.first()?);
            Some(doc! { col: { "$lt": val } })
        }
        "is less than or equal to" | "is on or before" => {
            let val = json_to_bson(filter.values.first()?);
            Some(doc! { col: { "$lte": val } })
        }
        "is greater than" | "is after" => {
            let val = json_to_bson(filter.values.first()?);
            Some(doc! { col: { "$gt": val } })
        }
        "is greater than or equal to" | "is on or after" => {
            let val = json_to_bson(filter.values.first()?);
            Some(doc! { col: { "$gte": val } })
        }
        "is between" => {
            if filter.values.len() >= 2 {
                let v1 = json_to_bson(&filter.values[0]);
                let v2 = json_to_bson(&filter.values[1]);
                Some(doc! { col: { "$gte": v1, "$lte": v2 } })
            } else {
                None
            }
        }
        "is not between" => {
            if filter.values.len() >= 2 {
                let v1 = json_to_bson(&filter.values[0]);
                let v2 = json_to_bson(&filter.values[1]);
                Some(doc! { "$or": [{ col: { "$lt": v1 } }, { col: { "$gt": v2 } }] })
            } else {
                None
            }
        }
        // Array operators
        "is any of" => {
            let vals: Vec<Bson> = filter.values.iter().map(json_to_bson).collect();
            if vals.is_empty() {
                None
            } else {
                Some(doc! { col: { "$in": vals } })
            }
        }
        "is none of" => {
            let vals: Vec<Bson> = filter.values.iter().map(json_to_bson).collect();
            if vals.is_empty() {
                None
            } else {
                Some(doc! { col: { "$nin": vals } })
            }
        }
        _ => None,
    }
}

/// Convert JSON value to BSON
fn json_to_bson(val: &serde_json::Value) -> mongodb::bson::Bson {
    use mongodb::bson::Bson;
    
    match val {
        serde_json::Value::Null => Bson::Null,
        serde_json::Value::Bool(b) => Bson::Boolean(*b),
        serde_json::Value::Number(n) => {
            if let Some(i) = n.as_i64() {
                Bson::Int64(i)
            } else if let Some(f) = n.as_f64() {
                Bson::Double(f)
            } else {
                Bson::Null
            }
        }
        serde_json::Value::String(s) => Bson::String(s.clone()),
        serde_json::Value::Array(arr) => {
            Bson::Array(arr.iter().map(json_to_bson).collect())
        }
        serde_json::Value::Object(obj) => {
            let doc: mongodb::bson::Document = obj
                .iter()
                .map(|(k, v)| (k.clone(), json_to_bson(v)))
                .collect();
            Bson::Document(doc)
        }
    }
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
    filters: Option<String>,
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

            // Parse filters
            let filter_params: Vec<FilterParam> = filters
                .as_ref()
                .and_then(|f| serde_json::from_str(f).ok())
                .unwrap_or_default();
            let where_clause = build_sql_where_clause(&filter_params);

            // Get total count (with filters)
            let count_query = format!("SELECT COUNT(*) as cnt FROM \"{}\"{}", table_name, where_clause);
            let count_row = sqlx::query(&count_query).fetch_one(pool).await?;
            let total_count: i64 = count_row.get("cnt");

            // Query data with pagination, sorting, and filters
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
                "SELECT * FROM \"{}\"{}{} LIMIT {} OFFSET {}",
                table_name, where_clause, order_clause, limit_val, offset_val
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
    filters: Option<String>,
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

            // Parse filters
            let filter_params: Vec<FilterParam> = filters
                .as_ref()
                .and_then(|f| serde_json::from_str(f).ok())
                .unwrap_or_default();
            let where_clause = build_sql_where_clause(&filter_params);

            // Get total count (with filters)
            let count_query = format!("SELECT COUNT(*) as cnt FROM \"{}\".\"{}\"{}", schema, table_name, where_clause);
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
                "SELECT * FROM \"{}\".\"{}\"{}{} LIMIT {} OFFSET {}",
                schema, table_name, where_clause, order_clause, limit_val, offset_val
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
    filters: Option<String>,
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
            
            // Parse filters and build MongoDB query
            let filter_params: Vec<FilterParam> = filters
                .as_ref()
                .and_then(|f| serde_json::from_str(f).ok())
                .unwrap_or_default();
            let query_doc = build_mongodb_filter(&filter_params);

            // Get total count (with filters)
            let total_count = collection.count_documents(query_doc.clone(), None).await? as i64;

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
            
            let mut cursor = collection.find(query_doc, find_options).await?;
            
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

// ============================================================================
// Raw Query Execution Commands
// ============================================================================

/// Execute a raw SQL query (for SQLite and PostgreSQL)
#[tauri::command]
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
        ConnectionPool::Sqlite(pool) => {
            execute_sqlite_raw(pool, &query).await
        }
        ConnectionPool::Postgres(pool) => {
            execute_postgres_raw(pool, &query).await
        }
        ConnectionPool::Mongo(_) => {
            Err(Error::InvalidDbUrl("Use execute_raw_mongo for MongoDB queries".to_string()))
        }
    }
}

/// Execute raw SQL on SQLite
async fn execute_sqlite_raw(
    pool: &sqlx::SqlitePool,
    query: &str,
) -> Result<QueryResult, Error> {
    let rows = sqlx::query(query).fetch_all(pool).await?;
    
    if rows.is_empty() {
        return Ok(QueryResult {
            columns: vec![],
            rows: vec![],
            total_count: Some(0),
        });
    }

    // Get column info from first row
    let first_row = &rows[0];
    let columns: Vec<ColumnInfo> = first_row
        .columns()
        .iter()
        .map(|col| ColumnInfo {
            name: col.name().to_string(),
            data_type: col.type_info().to_string(),
        })
        .collect();

    // Convert rows
    let result_rows: Vec<Vec<serde_json::Value>> = rows
        .iter()
        .map(|row| {
            columns
                .iter()
                .map(|col| sqlite_value_to_json(row, &col.name))
                .collect()
        })
        .collect();

    let row_count = result_rows.len() as i64;
    Ok(QueryResult {
        columns,
        rows: result_rows,
        total_count: Some(row_count),
    })
}

/// Execute raw SQL on PostgreSQL
async fn execute_postgres_raw(
    pool: &sqlx::PgPool,
    query: &str,
) -> Result<QueryResult, Error> {
    let rows = sqlx::query(query).fetch_all(pool).await?;
    
    if rows.is_empty() {
        return Ok(QueryResult {
            columns: vec![],
            rows: vec![],
            total_count: Some(0),
        });
    }

    // Get column info from first row
    let first_row = &rows[0];
    let columns: Vec<ColumnInfo> = first_row
        .columns()
        .iter()
        .map(|col| ColumnInfo {
            name: col.name().to_string(),
            data_type: col.type_info().to_string(),
        })
        .collect();

    // Convert rows
    let result_rows: Vec<Vec<serde_json::Value>> = rows
        .iter()
        .map(|row| {
            columns
                .iter()
                .map(|col| postgres_value_to_json(row, &col.name))
                .collect()
        })
        .collect();

    let row_count = result_rows.len() as i64;
    Ok(QueryResult {
        columns,
        rows: result_rows,
        total_count: Some(row_count),
    })
}

/// Execute a raw MongoDB query (find or aggregate)
#[tauri::command]
pub async fn execute_raw_mongo(
    project_path: String,
    conn_key: String,
    collection: String,
    query_type: String,  // "find" or "aggregate"
    query: String,       // JSON string
    registry: State<'_, ConnectionRegistry>,
    app: tauri::AppHandle,
) -> Result<QueryResult, Error> {
    use futures::TryStreamExt;

    let conn_id = ensure_connection(&project_path, &conn_key, &registry, app).await?;

    let pools = registry.pools().await;
    let pool = pools
        .get(&conn_id)
        .ok_or_else(|| Error::ConnectionNotFound(conn_id.clone()))?;

    match pool {
        ConnectionPool::Mongo(db) => {
            let coll = db.collection::<mongodb::bson::Document>(&collection);
            
            let documents: Vec<mongodb::bson::Document> = match query_type.as_str() {
                "find" => {
                    let filter: mongodb::bson::Document = serde_json::from_str(&query)
                        .map_err(|e| Error::InvalidDbUrl(format!("Invalid JSON filter: {}", e)))?;
                    
                    let mut cursor = coll.find(filter, None).await?;
                    let mut docs = Vec::new();
                    while let Some(doc) = cursor.try_next().await? {
                        docs.push(doc);
                    }
                    docs
                }
                "aggregate" => {
                    let pipeline: Vec<mongodb::bson::Document> = serde_json::from_str(&query)
                        .map_err(|e| Error::InvalidDbUrl(format!("Invalid JSON pipeline: {}", e)))?;
                    
                    let mut cursor = coll.aggregate(pipeline, None).await?;
                    let mut docs = Vec::new();
                    while let Some(doc) = cursor.try_next().await? {
                        docs.push(doc);
                    }
                    docs
                }
                _ => {
                    return Err(Error::InvalidDbUrl(format!("Unknown query type: {}. Use 'find' or 'aggregate'", query_type)));
                }
            };

            // Extract columns from first document
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
                rows: rows.clone(),
                total_count: Some(rows.len() as i64),
            })
        }
        _ => Err(Error::InvalidDbUrl("Expected MongoDB connection".to_string())),
    }
}
