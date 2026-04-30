// postgres/ — Fully specialized Postgres module.
// Nothing from here is shared with sqlite/ or mongodb/.
// Implemented in Phase 4.

pub mod data_viewer;
pub mod explain;
pub mod grammar;
pub mod inspector;
pub mod live_monitor;
pub mod mutations;
pub mod query_editor;
pub mod tree;
pub mod wizard;

use serde::{Deserialize, Serialize};

use crate::connection::lifecycle::{Connectable, TestReport};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PostgresConfig {
    pub label: String,
    pub host: String,
    pub port: u16,
    pub database: String,
    pub username: String,
    pub password: String,
    pub ssl_mode: SslMode,
}

#[derive(Debug, Clone, Copy, Default, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SslMode {
    #[default]
    Prefer,
    Require,
    Disable,
    VerifyCa,
    VerifyFull,
}

/// Live Postgres connection wrapping a sqlx pool.
pub struct PgConnection {
    pub config: PostgresConfig,
    // pool: sqlx::PgPool — added in Phase 4
}

impl Connectable for PgConnection {
    type Config = PostgresConfig;

    fn open(_config: Self::Config, _cx: &mut gpui::App) -> gpui::Task<anyhow::Result<Self>> {
        // TODO Phase 4
        gpui::Task::ready(Err(anyhow::anyhow!("Postgres engine not yet implemented")))
    }

    fn test(_config: &Self::Config, _cx: &mut gpui::App) -> gpui::Task<anyhow::Result<TestReport>> {
        // TODO Phase 4
        gpui::Task::ready(Err(anyhow::anyhow!("Postgres engine not yet implemented")))
    }

    async fn close(self) {
        // TODO Phase 4
    }
}
