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
use sqlx::postgres::{PgConnectOptions, PgPoolOptions, PgSslMode};
use sqlx::PgPool;

use crate::connection::lifecycle::{Connectable, TestReport};
use crate::tokio_bridge;

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

fn pg_ssl_mode(m: SslMode) -> PgSslMode {
    match m {
        SslMode::Disable => PgSslMode::Disable,
        SslMode::Prefer => PgSslMode::Prefer,
        SslMode::Require => PgSslMode::Require,
        SslMode::VerifyCa => PgSslMode::VerifyCa,
        SslMode::VerifyFull => PgSslMode::VerifyFull,
    }
}

pub(crate) fn pg_connect_options(config: &PostgresConfig) -> PgConnectOptions {
    PgConnectOptions::new()
        .host(&config.host)
        .port(config.port)
        .username(&config.username)
        .password(&config.password)
        .database(&config.database)
        .ssl_mode(pg_ssl_mode(config.ssl_mode))
}

/// Live Postgres connection wrapping a sqlx pool.
pub struct PgConnection {
    pub config: PostgresConfig,
    pub pool: PgPool,
}

impl Connectable for PgConnection {
    type Config = PostgresConfig;

    fn open(config: Self::Config, cx: &mut gpui::App) -> gpui::Task<anyhow::Result<Self>> {
        cx.background_executor().spawn(async move {
            tokio_bridge::block_on_db(async move {
                let opts = pg_connect_options(&config);
                let pool = PgPoolOptions::new()
                    .max_connections(8)
                    .connect_with(opts)
                    .await?;
                Ok(Self { config, pool })
            })
        })
    }

    fn test(config: &Self::Config, cx: &mut gpui::App) -> gpui::Task<anyhow::Result<TestReport>> {
        let config = config.clone();
        cx.background_executor().spawn(async move {
            tokio_bridge::block_on_db(async move {
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
        })
    }

    async fn close(self) {
        let pool = self.pool;
        tokio_bridge::block_on_db(async move {
            pool.close().await;
        });
    }
}
