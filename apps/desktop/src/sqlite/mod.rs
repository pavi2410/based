use std::path::{Path, PathBuf};
use std::time::Instant;

use gpui_kit::{App, Task};
// sqlite/ — GPUI panels + connection lifecycle; driver logic in `based-sqlite`.

pub mod attach_workspace;
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

use sqlx::{AssertSqlSafe, SqlitePool};

use crate::connection::lifecycle::{Connectable, TestReport};
use crate::db::Tokio;
use crate::project::ProjectRoot;

/// Resolve relative DB paths using the active Based project root when available.
pub fn resolve_sqlite_path(path: &Path, cx: &App) -> PathBuf {
    based_sqlite::resolve_sqlite_path(
        path,
        &SqlitePathContext {
            project_dir: cx.try_global::<ProjectRoot>().map(|p| p.0.clone()),
        },
    )
}

/// Live SQLite connection wrapping a sqlx pool.
pub struct SqliteConnection {
    pub pool: SqlitePool,
    pub server_version: Option<String>,
}

impl Connectable for SqliteConnection {
    type Config = SqliteConfig;

    fn open(config: Self::Config, cx: &mut App) -> Task<anyhow::Result<Self>> {
        let path = resolve_sqlite_path(&config.path, cx);
        Tokio::spawn_result(cx, async move {
            let pool = SqlitePool::connect_with(sqlite_connect_options(&SqliteOpenOptions {
                path: &path,
                read_only: config.read_only,
            }))
            .await?;
            apply_sqlite_pragmas(&pool, &config).await?;
            let version: String = sqlx::query_scalar("SELECT sqlite_version()")
                .fetch_one(&pool)
                .await?;
            Ok(Self {
                pool,
                server_version: Some(version),
            })
        })
    }

    fn test(config: &Self::Config, cx: &mut App) -> Task<anyhow::Result<TestReport>> {
        let config = config.clone();
        let path = resolve_sqlite_path(&config.path, cx);
        Tokio::spawn_result(cx, async move {
            let start = Instant::now();
            let pool = SqlitePool::connect_with(sqlite_connect_options(&SqliteOpenOptions {
                path: &path,
                read_only: config.read_only,
            }))
            .await?;
            let version: String = sqlx::query_scalar("SELECT sqlite_version()")
                .fetch_one(&pool)
                .await?;
            pool.close().await;
            Ok(TestReport {
                latency_ms: start.elapsed().as_millis() as u64,
                server_version: Some(version),
                message: None,
            })
        })
    }
}

async fn apply_sqlite_pragmas(pool: &SqlitePool, config: &SqliteConfig) -> anyhow::Result<()> {
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
