use std::path::{Path, PathBuf};
use std::time::Instant;

use gpui_kit::{App, Task};
// sqlite/ — GPUI panels + connection lifecycle; driver logic in `based-sqlite`.

mod eqp_parse;
pub mod eqp_viewer;
pub mod inspect;
pub mod pragmas;
pub mod query;
pub mod source;
pub mod tab_dispatch;

pub use based_sqlite::{
    SqliteConfig, SqliteOpenOptions, SqlitePathContext, SqlitePragma, sqlite_connect_options,
    sqlite_uri, sqlite3_command,
};

use based_core::ConnectError;
use based_sqlite::connect_error;
use sqlx::{AssertSqlSafe, SqlitePool};

use crate::connection::lifecycle::{Connectable, TestReport};
use crate::db::Tokio;
use crate::workspace::Workspaces;

/// Resolve relative DB paths using the active Based project root when available.
pub fn resolve_sqlite_path(path: &Path, cx: &App) -> PathBuf {
    based_sqlite::resolve_sqlite_path(
        path,
        &SqlitePathContext {
            // Connections open from the window the user is working in.
            project_dir: Workspaces::active_project(cx)
                .and_then(|p| p.root().map(ToOwned::to_owned)),
        },
    )
}

/// Live SQLite connection wrapping a sqlx pool.
pub struct SqliteConnection {
    pub pool: SqlitePool,
    pub server_version: Option<String>,
}

/// Open a pool on `path` and read the SQLite version.
async fn open_pool(path: &Path, read_only: bool) -> Result<(SqlitePool, String), ConnectError> {
    let pool = SqlitePool::connect_with(sqlite_connect_options(&SqliteOpenOptions {
        path,
        read_only,
    }))
    .await
    .map_err(connect_error)?;
    let version: String = sqlx::query_scalar("SELECT sqlite_version()")
        .fetch_one(&pool)
        .await
        .map_err(connect_error)?;
    Ok((pool, version))
}

impl Connectable for SqliteConnection {
    type Config = SqliteConfig;

    fn open(config: Self::Config, cx: &mut App) -> Task<Result<Self, ConnectError>> {
        let path = resolve_sqlite_path(&config.path, cx);
        Tokio::spawn_try(
            cx,
            async move {
                let (pool, version) = open_pool(&path, config.read_only).await?;
                apply_sqlite_pragmas(&pool, &config)
                    .await
                    .map_err(connect_error)?;
                Ok(Self {
                    pool,
                    server_version: Some(version),
                })
            },
            ConnectError::internal,
        )
    }

    fn test(config: &Self::Config, cx: &mut App) -> Task<Result<TestReport, ConnectError>> {
        let config = config.clone();
        let path = resolve_sqlite_path(&config.path, cx);
        Tokio::spawn_try(
            cx,
            async move {
                let start = Instant::now();
                let (pool, version) = open_pool(&path, config.read_only).await?;
                pool.close().await;
                Ok(TestReport {
                    latency_ms: start.elapsed().as_millis() as u64,
                    server_version: Some(version),
                    message: None,
                })
            },
            ConnectError::internal,
        )
    }
}

async fn apply_sqlite_pragmas(pool: &SqlitePool, config: &SqliteConfig) -> Result<(), sqlx::Error> {
    if let Some(p) = &config.pragma {
        if !config.read_only {
            sqlx::query(AssertSqlSafe(p.journal_mode.pragma()))
                .execute(pool)
                .await?;
            sqlx::query(AssertSqlSafe(p.synchronous.pragma()))
                .execute(pool)
                .await?;
        }
        sqlx::query(AssertSqlSafe(p.foreign_keys_pragma()))
            .execute(pool)
            .await?;
    }
    Ok(())
}
