// postgres/ — GPUI panels + connection lifecycle; driver logic in `based-postgres`.

pub mod data_viewer;
pub mod explain_plan;
pub mod grammar;
pub mod inspector;
pub mod live_monitor;
pub mod query_editor;
pub mod tab_dispatch;
pub mod tree;
pub mod wizard;

pub use based_postgres::{PostgresConfig, SslMode, execute_sql, pg_connect_options};

use sqlx::PgPool;
use sqlx::postgres::PgPoolOptions;

use crate::connection::lifecycle::{Connectable, TestReport};
use crate::db;
use gpui_tokio::Tokio;

/// Live Postgres connection wrapping a sqlx pool.
pub struct PgConnection {
    pub config: PostgresConfig,
    pub pool: PgPool,
    pub server_version: Option<String>,
}

impl Connectable for PgConnection {
    type Config = PostgresConfig;

    fn open(config: Self::Config, cx: &mut gpui::App) -> gpui::Task<anyhow::Result<Self>> {
        Tokio::spawn_result(cx, async move {
            let opts = pg_connect_options(&config);
            let pool = PgPoolOptions::new()
                .max_connections(8)
                .connect_with(opts)
                .await?;
            let version: String = sqlx::query_scalar("SELECT version()")
                .fetch_one(&pool)
                .await?;
            let short = version.lines().next().unwrap_or(&version).to_string();
            Ok(Self {
                config,
                pool,
                server_version: Some(short),
            })
        })
    }

    fn test(config: &Self::Config, cx: &mut gpui::App) -> gpui::Task<anyhow::Result<TestReport>> {
        let config = config.clone();
        Tokio::spawn_result(cx, async move {
            let start = std::time::Instant::now();
            let opts = pg_connect_options(&config);
            let pool = PgPoolOptions::new()
                .max_connections(1)
                .connect_with(opts)
                .await?;
            let version: String = sqlx::query_scalar("SELECT version()")
                .fetch_one(&pool)
                .await?;
            pool.close().await;
            let short = version.lines().next().unwrap_or(&version).to_string();
            Ok(TestReport {
                latency_ms: start.elapsed().as_millis() as u64,
                server_version: Some(short),
                message: None,
            })
        })
    }

    async fn close(self) {
        db::close_pg_pool(self.pool);
    }
}

use crate::connection::descriptor::EngineDescriptor;
use based_core::EngineKind;

/// Engine descriptor for PostgreSQL — registered at startup via [`crate::connection::EngineRegistry`].
pub struct PostgresEngine;

impl EngineDescriptor for PostgresEngine {
    fn kind(&self) -> EngineKind {
        EngineKind::Postgres
    }
    fn display_name(&self) -> &str {
        "PostgreSQL"
    }
    fn icon_name(&self) -> &str {
        "postgres"
    }
    fn default_port(&self) -> Option<u16> {
        Some(5432)
    }
    fn supports_tab_kind(&self, kind: &str) -> bool {
        matches!(
            kind,
            "query_editor" | "data_viewer" | "inspector" | "object_info" | "dashboard"
        )
    }
}
