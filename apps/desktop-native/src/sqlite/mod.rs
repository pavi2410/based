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

use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};
use sqlx::sqlite::SqliteConnectOptions;
use sqlx::SqlitePool;

use crate::connection::lifecycle::{Connectable, TestReport};
use crate::db;
use gpui_tokio::Tokio;

/// Walk parents of `std::env::current_dir()` for a directory that contains `.based/`.
fn based_project_root() -> Option<PathBuf> {
    let mut dir = std::env::current_dir().ok()?;
    loop {
        if dir.join(".based").is_dir() {
            return Some(dir);
        }
        dir = dir.parent()?.to_path_buf();
    }
}

/// Resolve relative DB paths against `BASED_PROJECT_DIR`, then the Based project root (`.based/`
/// ancestor), then the process working directory. Absolute paths are unchanged.
///
/// Relative paths like `app.db` from the UI therefore open next to the repo instead of depending
/// on an unpredictable GUI process CWD (which commonly causes SQLite error 14).
pub fn resolve_sqlite_path(path: &Path) -> PathBuf {
    if path.is_absolute() {
        return path.to_path_buf();
    }
    if let Ok(dir) = std::env::var("BASED_PROJECT_DIR") {
        return PathBuf::from(dir).join(path);
    }
    if let Some(root) = based_project_root() {
        return root.join(path);
    }
    std::env::current_dir()
        .unwrap_or_else(|_| PathBuf::from("."))
        .join(path)
}

fn sqlite_options(path: &Path, create_if_missing: bool) -> SqliteConnectOptions {
    let mut opts = SqliteConnectOptions::new().filename(path);
    if create_if_missing {
        opts = opts.create_if_missing(true);
    }
    opts
}

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
        Tokio::spawn_result(cx, async move {
            let path = resolve_sqlite_path(&config.path);
            let create = !path.exists();
            let pool = SqlitePool::connect_with(sqlite_options(&path, create)).await?;
            if config.wal {
                sqlx::query("PRAGMA journal_mode=WAL")
                    .execute(&pool)
                    .await?;
            }
            Ok(Self { config, pool })
        })
    }

    fn test(
        config: &Self::Config,
        cx: &mut gpui::App,
    ) -> gpui::Task<anyhow::Result<TestReport>> {
        let config = config.clone();
        Tokio::spawn_result(cx, async move {
            let path = resolve_sqlite_path(&config.path);
            let start = std::time::Instant::now();
            let pool = SqlitePool::connect_with(sqlite_options(&path, false)).await?;
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
    }

    async fn close(self) {
        db::close_sqlite_pool(self.pool);
    }
}
