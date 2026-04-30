// sqlite/ — Fully specialized SQLite module.
// Nothing from here is shared with postgres/ or mongodb/.
// Implemented in Phase 3.

pub mod attach_workspace;
pub mod data_viewer;
pub mod eqp_viewer;
pub mod fts_console;
pub mod inspector;
pub mod mutations;
pub mod pragma_browser;
pub mod query_editor;
pub mod tree;
pub mod wizard;

use serde::{Deserialize, Serialize};
use sqlx::SqlitePool;

use crate::connection::lifecycle::{Connectable, TestReport};
use crate::tokio_bridge;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SqliteConfig {
    pub label: String,
    pub path: std::path::PathBuf,
    /// true = WAL mode; false = journal mode (default)
    pub wal: bool,
}

/// Live SQLite connection wrapping a sqlx pool.
pub struct SqliteConnection {
    pub config: SqliteConfig,
    pub pool: SqlitePool,
}

impl Connectable for SqliteConnection {
    type Config = SqliteConfig;

    fn open(config: Self::Config, cx: &mut gpui::App) -> gpui::Task<anyhow::Result<Self>> {
        cx.background_executor().spawn(async move {
            tokio_bridge::block_on_db(async move {
                let url = format!("sqlite:{}", config.path.display());
                let pool = SqlitePool::connect(&url).await?;
                if config.wal {
                    sqlx::query("PRAGMA journal_mode=WAL")
                        .execute(&pool)
                        .await?;
                }
                Ok(Self { config, pool })
            })
        })
    }

    fn test(
        config: &Self::Config,
        cx: &mut gpui::App,
    ) -> gpui::Task<anyhow::Result<TestReport>> {
        let config = config.clone();
        cx.background_executor().spawn(async move {
            tokio_bridge::block_on_db(async move {
                let url = format!("sqlite:{}", config.path.display());
                let start = std::time::Instant::now();
                let pool = SqlitePool::connect(&url).await?;
                let version: String =
                    sqlx::query_scalar("SELECT sqlite_version()")
                        .fetch_one(&pool)
                        .await?;
                pool.close().await;
                Ok(TestReport {
                    latency_ms: start.elapsed().as_millis() as u64,
                    server_version: Some(version),
                    message: None,
                })
            })
        })
    }

    async fn close(self) {
        let pool = self.pool;
        tokio_bridge::block_on_db(async move {
            pool.close().await;
        });
    }
}
