// sqlite/ — GPUI panels + connection lifecycle; driver logic in `based-sqlite`.

pub mod attach_workspace;
pub mod data_viewer;
mod eqp_parse;
pub mod eqp_viewer;
pub mod fts_console;
pub mod inspector;
pub mod pragma_browser;
pub mod query_editor;
pub mod tree;
pub mod wizard;

pub use based_sqlite::{SqliteConfig, SqlitePathContext, sqlite_connect_options};

use sqlx::{AssertSqlSafe, SqlitePool};

use crate::connection::lifecycle::{Connectable, TestReport};
use crate::db;
use crate::project::ProjectRoot;
use gpui_tokio::Tokio;

/// Resolve relative DB paths using the active Based project root when available.
pub fn resolve_sqlite_path(path: &std::path::Path, cx: &gpui::App) -> std::path::PathBuf {
    based_sqlite::resolve_sqlite_path(
        path,
        &SqlitePathContext {
            project_dir: cx.try_global::<ProjectRoot>().map(|p| p.0.clone()),
        },
    )
}

/// Live SQLite connection wrapping a sqlx pool.
pub struct SqliteConnection {
    pub config: SqliteConfig,
    pub pool: SqlitePool,
    pub server_version: Option<String>,
}

impl Connectable for SqliteConnection {
    type Config = SqliteConfig;

    fn open(config: Self::Config, cx: &mut gpui::App) -> gpui::Task<anyhow::Result<Self>> {
        let path = resolve_sqlite_path(&config.path, cx);
        Tokio::spawn_result(cx, async move {
            let create = !path.exists();
            let pool = SqlitePool::connect_with(sqlite_connect_options(&path, create)).await?;
            apply_sqlite_pragmas(&pool, &config).await?;
            let version: String = sqlx::query_scalar("SELECT sqlite_version()")
                .fetch_one(&pool)
                .await?;
            Ok(Self {
                config,
                pool,
                server_version: Some(version),
            })
        })
    }

    fn test(config: &Self::Config, cx: &mut gpui::App) -> gpui::Task<anyhow::Result<TestReport>> {
        let config = config.clone();
        let path = resolve_sqlite_path(&config.path, cx);
        Tokio::spawn_result(cx, async move {
            let start = std::time::Instant::now();
            let pool = SqlitePool::connect_with(sqlite_connect_options(&path, false)).await?;
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

    async fn close(self) {
        db::close_sqlite_pool(self.pool);
    }
}

async fn apply_sqlite_pragmas(pool: &SqlitePool, config: &SqliteConfig) -> anyhow::Result<()> {
    if let Some(p) = &config.pragma {
        let journal = journal_mode_pragma(&p.journal_mode)?;
        sqlx::query(AssertSqlSafe(journal)).execute(pool).await?;
        let sync = synchronous_pragma(&p.synchronous)?;
        sqlx::query(AssertSqlSafe(sync)).execute(pool).await?;
        let fk = if p.foreign_keys {
            "PRAGMA foreign_keys=ON"
        } else {
            "PRAGMA foreign_keys=OFF"
        };
        sqlx::query(AssertSqlSafe(fk)).execute(pool).await?;
    }
    Ok(())
}

fn journal_mode_pragma(mode: &str) -> anyhow::Result<&'static str> {
    match mode.to_ascii_lowercase().as_str() {
        "wal" => Ok("PRAGMA journal_mode=WAL"),
        "delete" => Ok("PRAGMA journal_mode=DELETE"),
        "truncate" => Ok("PRAGMA journal_mode=TRUNCATE"),
        "persist" => Ok("PRAGMA journal_mode=PERSIST"),
        "memory" => Ok("PRAGMA journal_mode=MEMORY"),
        "off" => Ok("PRAGMA journal_mode=OFF"),
        other => anyhow::bail!("unsupported journal_mode: {other}"),
    }
}

fn synchronous_pragma(mode: &str) -> anyhow::Result<&'static str> {
    match mode.to_ascii_lowercase().as_str() {
        "off" => Ok("PRAGMA synchronous=OFF"),
        "normal" => Ok("PRAGMA synchronous=NORMAL"),
        "full" => Ok("PRAGMA synchronous=FULL"),
        "extra" => Ok("PRAGMA synchronous=EXTRA"),
        other => anyhow::bail!("unsupported synchronous: {other}"),
    }
}
