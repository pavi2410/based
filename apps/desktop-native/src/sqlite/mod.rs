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

use crate::connection::lifecycle::{Connectable, TestReport};

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
    // pool: sqlx::SqlitePool — added in Phase 3
}

impl Connectable for SqliteConnection {
    type Config = SqliteConfig;

    fn open(_config: Self::Config, _cx: &mut gpui::App) -> gpui::Task<anyhow::Result<Self>> {
        // TODO Phase 3
        gpui::Task::ready(Err(anyhow::anyhow!("SQLite engine not yet implemented")))
    }

    fn test(_config: &Self::Config, _cx: &mut gpui::App) -> gpui::Task<anyhow::Result<TestReport>> {
        // TODO Phase 3
        gpui::Task::ready(Err(anyhow::anyhow!("SQLite engine not yet implemented")))
    }

    async fn close(self) {
        // TODO Phase 3
    }
}
