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
use crate::engine::types::TableDescription;
use crate::engine::{self, BrowseOptions, QueryResult};
use crate::error::Error;
use crate::project_commands::read_project_config;
use crate::project_types::{ConnectionConfig, Engine};
use crate::query_registry::QueryRegistry;
use crate::schema_cache::{CachedObjectKey, SchemaCache};
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

/// Probe a connection config without registering it anywhere.
///
/// The connection wizard uses this for its "Test connection" button
/// so the user gets immediate feedback before committing the config
/// to `config.toml`. We intentionally do not cache or pool the
/// resulting connection: the probe runs, we drop the pool, and a
/// subsequent regular `connect_project_db` re-establishes a pool we
/// actually track.
#[tauri::command]
#[specta::specta]
pub async fn test_connection(
    project_path: String,
    conn_config: ConnectionConfig,
    app: tauri::AppHandle,
) -> Result<(), Error> {
    let env_vars = load_project_env(&project_path);
    let conn_string = build_connection_string(&conn_config, &project_path, &env_vars)?;
    let pool = ConnectionPool::connect(&conn_string, &app).await?;
    // A one-shot roundtrip to fail fast on credential errors that
    // connect() might miss (e.g. lazy-connecting drivers).
    match pool {
        ConnectionPool::Sqlite(ref p) => {
            sqlx::query("SELECT 1").execute(p).await?;
        }
        ConnectionPool::Postgres(ref p) => {
            sqlx::query("SELECT 1").execute(p).await?;
        }
        ConnectionPool::Mongo(ref db) => {
            // MongoDB doesn't do a single-statement ping through our
            // abstraction; listing collection names is cheap and
            // equally diagnostic.
            use mongodb::bson::doc;
            db.run_command(doc! {"ping": 1}, None).await?;
        }
    }
    drop(pool);
    Ok(())
}

/// Refuse any write operation against a connection flagged as readonly
/// in `config.toml`. This is a UI safeguard — the DB-level flag (e.g.
/// opening SQLite with `mode=ro`) is the real enforcement; this stops
/// mutations from even being issued, so users who flip the switch get
/// an immediate, understandable error instead of whatever the driver
/// would surface several layers later.
async fn ensure_writable(project_path: &str, conn_key: &str) -> Result<(), Error> {
    let config = read_project_config(project_path.to_string())
        .await
        .map_err(Error::InvalidDbUrl)?;
    let conn_config = config
        .connection
        .get(conn_key)
        .ok_or_else(|| Error::InvalidDbUrl(format!("Connection key not found: {}", conn_key)))?;
    if conn_config.readonly.unwrap_or(false) {
        return Err(Error::InvalidDbUrl(format!(
            "Connection '{}' is marked readonly in config.toml",
            conn_key
        )));
    }
    Ok(())
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
    cache: State<'_, SchemaCache>,
) -> Result<(), Error> {
    let id = ConnectionRegistry::get_id(&project_path, &conn_key);
    cache.invalidate_connection(id.as_str()).await;
    registry.close(&id).await;
    Ok(())
}

#[tauri::command]
#[specta::specta]
pub async fn close_project_connections(
    project_path: String,
    registry: State<'_, ConnectionRegistry>,
    cache: State<'_, SchemaCache>,
) -> Result<(), Error> {
    // Cache is scoped by connection id; blow it away for every
    // connection in this project so a reconnect surfaces fresh schema.
    for conn_key in registry.project_conn_keys(&project_path).await {
        let id = ConnectionRegistry::get_id(&project_path, &conn_key);
        cache.invalidate_connection(id.as_str()).await;
    }
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
// Schema inspector commands
// ---------------------------------------------------------------------------

/// Describe a SQLite table (columns, indexes, foreign keys).
#[tauri::command]
#[specta::specta]
pub async fn describe_sqlite_table(
    project_path: String,
    conn_key: String,
    table_name: String,
    registry: State<'_, ConnectionRegistry>,
    cache: State<'_, SchemaCache>,
    app: tauri::AppHandle,
) -> Result<TableDescription, Error> {
    let conn_id = ensure_connection(&project_path, &conn_key, &registry, app).await?;
    let key = CachedObjectKey::new(None, &table_name);
    if let Some(hit) = cache.get(&conn_id, &key).await {
        return Ok((*hit).clone());
    }
    let pools = registry.pools().await;
    let pool = pools
        .get(&conn_id)
        .ok_or_else(|| Error::ConnectionNotFound(conn_id.clone()))?;
    let desc = match pool {
        ConnectionPool::Sqlite(p) => engine::sqlite::describe_table(p, &table_name).await?,
        _ => return Err(expected("SQLite")),
    };
    drop(pools);
    cache.put(&conn_id, key, desc.clone()).await;
    Ok(desc)
}

/// Describe a PostgreSQL table (columns, indexes, foreign keys,
/// reltuples estimate).
#[tauri::command]
#[specta::specta]
pub async fn describe_postgres_table(
    project_path: String,
    conn_key: String,
    schema: String,
    table_name: String,
    registry: State<'_, ConnectionRegistry>,
    cache: State<'_, SchemaCache>,
    app: tauri::AppHandle,
) -> Result<TableDescription, Error> {
    let conn_id = ensure_connection(&project_path, &conn_key, &registry, app).await?;
    let key = CachedObjectKey::new(Some(&schema), &table_name);
    if let Some(hit) = cache.get(&conn_id, &key).await {
        return Ok((*hit).clone());
    }
    let pools = registry.pools().await;
    let pool = pools
        .get(&conn_id)
        .ok_or_else(|| Error::ConnectionNotFound(conn_id.clone()))?;
    let desc = match pool {
        ConnectionPool::Postgres(p) => {
            engine::postgres::describe_table(p, &schema, &table_name).await?
        }
        _ => return Err(expected("PostgreSQL")),
    };
    drop(pools);
    cache.put(&conn_id, key, desc.clone()).await;
    Ok(desc)
}

/// Describe a MongoDB collection (sampled columns + real indexes +
/// estimated row count).
#[tauri::command]
#[specta::specta]
pub async fn describe_mongodb_collection(
    project_path: String,
    conn_key: String,
    collection_name: String,
    registry: State<'_, ConnectionRegistry>,
    cache: State<'_, SchemaCache>,
    app: tauri::AppHandle,
) -> Result<TableDescription, Error> {
    let conn_id = ensure_connection(&project_path, &conn_key, &registry, app).await?;
    let key = CachedObjectKey::new(None, &collection_name);
    if let Some(hit) = cache.get(&conn_id, &key).await {
        return Ok((*hit).clone());
    }
    let pools = registry.pools().await;
    let pool = pools
        .get(&conn_id)
        .ok_or_else(|| Error::ConnectionNotFound(conn_id.clone()))?;
    let desc = match pool {
        ConnectionPool::Mongo(db) => {
            engine::mongo::describe_collection(db, &collection_name).await?
        }
        _ => return Err(expected("MongoDB")),
    };
    drop(pools);
    cache.put(&conn_id, key, desc.clone()).await;
    Ok(desc)
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
    token: Option<String>,
    timeout_ms: Option<u64>,
    registry: State<'_, ConnectionRegistry>,
    cache: State<'_, SchemaCache>,
    queries: State<'_, QueryRegistry>,
    app: tauri::AppHandle,
) -> Result<QueryResult, Error> {
    let conn_id = ensure_connection(&project_path, &conn_key, &registry, app).await?;

    // Register with the query registry BEFORE acquiring the pool guard.
    // The handle drops naturally with `finish_by_id` in the finally-
    // style block below regardless of how the query resolves.
    let cancel_handle = if let Some(ref t) = token {
        Some(queries.register_with(t.clone()).await)
    } else {
        None
    };

    let pools = registry.pools().await;
    let pool = pools
        .get(&conn_id)
        .ok_or_else(|| Error::ConnectionNotFound(conn_id.clone()))?;
    let fut = async {
        match pool {
            ConnectionPool::Sqlite(p) => engine::sqlite::execute_raw(p, &query).await,
            ConnectionPool::Postgres(p) => engine::postgres::execute_raw(p, &query).await,
            ConnectionPool::Mongo(_) => Err(Error::InvalidDbUrl(
                "Use execute_raw_mongo for MongoDB queries".to_string(),
            )),
        }
    };

    let result = run_with_cancel(fut, cancel_handle.as_ref(), timeout_ms).await;

    drop(pools);
    if let Some(ref t) = token {
        queries.finish_by_id(t).await;
    }

    // Raw SQL may have been DDL; we don't parse the statement so we
    // conservatively invalidate the whole connection's schema cache
    // rather than try to be clever. The cost is one extra describe
    // roundtrip on the next Structure view.
    if result.is_ok() {
        cache.invalidate_connection(&conn_id).await;
    }
    result
}

/// Race a query future against cancellation + timeout.
///
/// Keeping this in a tiny helper means the two execute commands can
/// share the same semantics without every call site reimplementing
/// the `tokio::select!` dance. The ordering is important:
///   1. If there's no cancellation handle the wrapper collapses to
///      a timeout-only wait (or a plain `.await` if no timeout).
///   2. Cancellation is checked via `handle.cancelled().await` which
///      returns immediately if already cancelled, so a racing
///      `cancel_query` that lands before we enter the select is still
///      honoured.
///   3. Dropping the inner future is how we actually free engine
///      resources (sqlx / mongodb both respect future drop).
async fn run_with_cancel<F>(
    fut: F,
    cancel: Option<&crate::query_registry::CancellationHandle>,
    timeout_ms: Option<u64>,
) -> Result<QueryResult, Error>
where
    F: std::future::Future<Output = Result<QueryResult, Error>>,
{
    use tokio::time::{Duration, timeout};
    let inner = async {
        match cancel {
            Some(handle) => {
                tokio::select! {
                    r = fut => r,
                    _ = handle.cancelled() => Err(Error::Cancelled),
                }
            }
            None => fut.await,
        }
    };
    match timeout_ms {
        Some(ms) if ms > 0 => match timeout(Duration::from_millis(ms), inner).await {
            Ok(r) => r,
            Err(_) => Err(Error::Timeout(ms)),
        },
        _ => inner.await,
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
    token: Option<String>,
    timeout_ms: Option<u64>,
    registry: State<'_, ConnectionRegistry>,
    queries: State<'_, QueryRegistry>,
    app: tauri::AppHandle,
) -> Result<QueryResult, Error> {
    let conn_id = ensure_connection(&project_path, &conn_key, &registry, app).await?;

    let cancel_handle = if let Some(ref t) = token {
        Some(queries.register_with(t.clone()).await)
    } else {
        None
    };

    let pools = registry.pools().await;
    let pool = pools
        .get(&conn_id)
        .ok_or_else(|| Error::ConnectionNotFound(conn_id.clone()))?;
    let fut = async {
        match pool {
            ConnectionPool::Mongo(db) => {
                engine::mongo::execute_raw(db, &collection, &query_type, &query).await
            }
            _ => Err(expected("MongoDB")),
        }
    };
    let result = run_with_cancel(fut, cancel_handle.as_ref(), timeout_ms).await;
    drop(pools);
    if let Some(ref t) = token {
        queries.finish_by_id(t).await;
    }
    result
}

// ---------------------------------------------------------------------------
// Row-level mutation commands (update / insert / delete)
// ---------------------------------------------------------------------------
//
// The frontend sends primary-key predicates and change sets as
// `Record<string, JsonValue>`, which round-trips to
// `serde_json::Map<String, Value>`. That shape is intentionally
// schemaless: the UI infers which columns are primary keys from the
// `describe_*` result and bundles them into `pk`, so the backend
// doesn't need its own PK discovery step here.

type RowMap = serde_json::Map<String, serde_json::Value>;

/// Update a single SQLite row. `pk` must identify exactly one row; we
/// return the number of rows actually changed so the UI can catch the
/// "someone else deleted it between browse and edit" case.
#[tauri::command]
#[specta::specta]
pub async fn update_sqlite_row(
    project_path: String,
    conn_key: String,
    table_name: String,
    pk: RowMap,
    changes: RowMap,
    registry: State<'_, ConnectionRegistry>,
    cache: State<'_, SchemaCache>,
    app: tauri::AppHandle,
) -> Result<u64, Error> {
    ensure_writable(&project_path, &conn_key).await?;
    let conn_id = ensure_connection(&project_path, &conn_key, &registry, app).await?;
    let pools = registry.pools().await;
    let pool = pools
        .get(&conn_id)
        .ok_or_else(|| Error::ConnectionNotFound(conn_id.clone()))?;
    let result = match pool {
        ConnectionPool::Sqlite(p) => {
            engine::mutations::sqlite_update_row(p, &table_name, &pk, &changes).await
        }
        _ => return Err(expected("SQLite")),
    };
    drop(pools);
    cache
        .invalidate(&conn_id, &CachedObjectKey::new(None, &table_name))
        .await;
    result
}

#[tauri::command]
#[specta::specta]
pub async fn insert_sqlite_row(
    project_path: String,
    conn_key: String,
    table_name: String,
    values: RowMap,
    registry: State<'_, ConnectionRegistry>,
    cache: State<'_, SchemaCache>,
    app: tauri::AppHandle,
) -> Result<u64, Error> {
    ensure_writable(&project_path, &conn_key).await?;
    let conn_id = ensure_connection(&project_path, &conn_key, &registry, app).await?;
    let pools = registry.pools().await;
    let pool = pools
        .get(&conn_id)
        .ok_or_else(|| Error::ConnectionNotFound(conn_id.clone()))?;
    let result = match pool {
        ConnectionPool::Sqlite(p) => {
            engine::mutations::sqlite_insert_row(p, &table_name, &values).await
        }
        _ => return Err(expected("SQLite")),
    };
    drop(pools);
    cache
        .invalidate(&conn_id, &CachedObjectKey::new(None, &table_name))
        .await;
    result
}

#[tauri::command]
#[specta::specta]
pub async fn delete_sqlite_row(
    project_path: String,
    conn_key: String,
    table_name: String,
    pk: RowMap,
    registry: State<'_, ConnectionRegistry>,
    cache: State<'_, SchemaCache>,
    app: tauri::AppHandle,
) -> Result<u64, Error> {
    ensure_writable(&project_path, &conn_key).await?;
    let conn_id = ensure_connection(&project_path, &conn_key, &registry, app).await?;
    let pools = registry.pools().await;
    let pool = pools
        .get(&conn_id)
        .ok_or_else(|| Error::ConnectionNotFound(conn_id.clone()))?;
    let result = match pool {
        ConnectionPool::Sqlite(p) => {
            engine::mutations::sqlite_delete_row(p, &table_name, &pk).await
        }
        _ => return Err(expected("SQLite")),
    };
    drop(pools);
    cache
        .invalidate(&conn_id, &CachedObjectKey::new(None, &table_name))
        .await;
    result
}

#[tauri::command]
#[specta::specta]
pub async fn update_postgres_row(
    project_path: String,
    conn_key: String,
    schema: String,
    table_name: String,
    pk: RowMap,
    changes: RowMap,
    registry: State<'_, ConnectionRegistry>,
    cache: State<'_, SchemaCache>,
    app: tauri::AppHandle,
) -> Result<u64, Error> {
    ensure_writable(&project_path, &conn_key).await?;
    let conn_id = ensure_connection(&project_path, &conn_key, &registry, app).await?;
    let pools = registry.pools().await;
    let pool = pools
        .get(&conn_id)
        .ok_or_else(|| Error::ConnectionNotFound(conn_id.clone()))?;
    let result = match pool {
        ConnectionPool::Postgres(p) => {
            engine::mutations::postgres_update_row(p, &schema, &table_name, &pk, &changes).await
        }
        _ => return Err(expected("PostgreSQL")),
    };
    drop(pools);
    cache
        .invalidate(&conn_id, &CachedObjectKey::new(Some(&schema), &table_name))
        .await;
    result
}

#[tauri::command]
#[specta::specta]
pub async fn insert_postgres_row(
    project_path: String,
    conn_key: String,
    schema: String,
    table_name: String,
    values: RowMap,
    registry: State<'_, ConnectionRegistry>,
    cache: State<'_, SchemaCache>,
    app: tauri::AppHandle,
) -> Result<u64, Error> {
    ensure_writable(&project_path, &conn_key).await?;
    let conn_id = ensure_connection(&project_path, &conn_key, &registry, app).await?;
    let pools = registry.pools().await;
    let pool = pools
        .get(&conn_id)
        .ok_or_else(|| Error::ConnectionNotFound(conn_id.clone()))?;
    let result = match pool {
        ConnectionPool::Postgres(p) => {
            engine::mutations::postgres_insert_row(p, &schema, &table_name, &values).await
        }
        _ => return Err(expected("PostgreSQL")),
    };
    drop(pools);
    cache
        .invalidate(&conn_id, &CachedObjectKey::new(Some(&schema), &table_name))
        .await;
    result
}

#[tauri::command]
#[specta::specta]
pub async fn delete_postgres_row(
    project_path: String,
    conn_key: String,
    schema: String,
    table_name: String,
    pk: RowMap,
    registry: State<'_, ConnectionRegistry>,
    cache: State<'_, SchemaCache>,
    app: tauri::AppHandle,
) -> Result<u64, Error> {
    ensure_writable(&project_path, &conn_key).await?;
    let conn_id = ensure_connection(&project_path, &conn_key, &registry, app).await?;
    let pools = registry.pools().await;
    let pool = pools
        .get(&conn_id)
        .ok_or_else(|| Error::ConnectionNotFound(conn_id.clone()))?;
    let result = match pool {
        ConnectionPool::Postgres(p) => {
            engine::mutations::postgres_delete_row(p, &schema, &table_name, &pk).await
        }
        _ => return Err(expected("PostgreSQL")),
    };
    drop(pools);
    cache
        .invalidate(&conn_id, &CachedObjectKey::new(Some(&schema), &table_name))
        .await;
    result
}

#[tauri::command]
#[specta::specta]
pub async fn update_mongodb_document(
    project_path: String,
    conn_key: String,
    collection_name: String,
    pk: RowMap,
    changes: RowMap,
    registry: State<'_, ConnectionRegistry>,
    cache: State<'_, SchemaCache>,
    app: tauri::AppHandle,
) -> Result<u64, Error> {
    ensure_writable(&project_path, &conn_key).await?;
    let conn_id = ensure_connection(&project_path, &conn_key, &registry, app).await?;
    let pools = registry.pools().await;
    let pool = pools
        .get(&conn_id)
        .ok_or_else(|| Error::ConnectionNotFound(conn_id.clone()))?;
    let result = match pool {
        ConnectionPool::Mongo(db) => {
            engine::mutations::mongodb_update_document(db, &collection_name, &pk, &changes).await
        }
        _ => return Err(expected("MongoDB")),
    };
    drop(pools);
    cache
        .invalidate(&conn_id, &CachedObjectKey::new(None, &collection_name))
        .await;
    result
}

#[tauri::command]
#[specta::specta]
pub async fn insert_mongodb_document(
    project_path: String,
    conn_key: String,
    collection_name: String,
    values: RowMap,
    registry: State<'_, ConnectionRegistry>,
    cache: State<'_, SchemaCache>,
    app: tauri::AppHandle,
) -> Result<u64, Error> {
    ensure_writable(&project_path, &conn_key).await?;
    let conn_id = ensure_connection(&project_path, &conn_key, &registry, app).await?;
    let pools = registry.pools().await;
    let pool = pools
        .get(&conn_id)
        .ok_or_else(|| Error::ConnectionNotFound(conn_id.clone()))?;
    let result = match pool {
        ConnectionPool::Mongo(db) => {
            engine::mutations::mongodb_insert_document(db, &collection_name, &values).await
        }
        _ => return Err(expected("MongoDB")),
    };
    drop(pools);
    cache
        .invalidate(&conn_id, &CachedObjectKey::new(None, &collection_name))
        .await;
    result
}

#[tauri::command]
#[specta::specta]
pub async fn delete_mongodb_document(
    project_path: String,
    conn_key: String,
    collection_name: String,
    pk: RowMap,
    registry: State<'_, ConnectionRegistry>,
    cache: State<'_, SchemaCache>,
    app: tauri::AppHandle,
) -> Result<u64, Error> {
    ensure_writable(&project_path, &conn_key).await?;
    let conn_id = ensure_connection(&project_path, &conn_key, &registry, app).await?;
    let pools = registry.pools().await;
    let pool = pools
        .get(&conn_id)
        .ok_or_else(|| Error::ConnectionNotFound(conn_id.clone()))?;
    let result = match pool {
        ConnectionPool::Mongo(db) => {
            engine::mutations::mongodb_delete_document(db, &collection_name, &pk).await
        }
        _ => return Err(expected("MongoDB")),
    };
    drop(pools);
    cache
        .invalidate(&conn_id, &CachedObjectKey::new(None, &collection_name))
        .await;
    result
}

// ---------------------------------------------------------------------------
// Query cancellation
// ---------------------------------------------------------------------------

/// Cancel an in-flight query by token. The actual mid-query checks
/// land with the Phase 2 "params + history + cancel" work; this
/// command is wired up now so the frontend can start carrying the
/// token round-trip through the editor.
#[tauri::command]
#[specta::specta]
pub async fn cancel_query(token: String, queries: State<'_, QueryRegistry>) -> Result<bool, Error> {
    Ok(queries.cancel(&token).await)
}
