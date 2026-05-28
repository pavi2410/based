// sqlite/ — GPUI panels + connection lifecycle; driver logic in `based-sqlite`.

pub mod attach_workspace;
pub mod data_viewer;
mod eqp_parse;
pub mod eqp_viewer;
pub mod fts_console;
pub mod inspector;
pub mod mutations;
pub mod pragma_browser;
pub mod query_editor;
pub mod tree;
pub mod wizard;

pub use based_sqlite::{SqliteConfig, SqlitePathContext, sqlite_connect_options};

use sqlx::SqlitePool;

use crate::connection::lifecycle::{Connectable, TestReport};
use crate::db;
use crate::project::find_project_root;
use gpui_tokio::Tokio;

/// Resolve relative DB paths using the current Based project root when available.
pub fn resolve_sqlite_path(path: &std::path::Path) -> std::path::PathBuf {
    based_sqlite::resolve_sqlite_path(
        path,
        &SqlitePathContext {
            project_dir: find_project_root(),
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
        Tokio::spawn_result(cx, async move {
            let path = resolve_sqlite_path(&config.path);
            let create = !path.exists();
            let pool = SqlitePool::connect_with(sqlite_connect_options(&path, create)).await?;
            if config.wal {
                sqlx::query("PRAGMA journal_mode=WAL")
                    .execute(&pool)
                    .await?;
            }
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
        Tokio::spawn_result(cx, async move {
            let path = resolve_sqlite_path(&config.path);
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
