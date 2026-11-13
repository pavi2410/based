use std::time::{Duration, Instant};

use gpui_kit::{App, Task};
// postgres/ — GPUI panels + connection lifecycle; driver logic in `based-postgres`.

pub mod explain_plan;
pub mod inspect;
pub mod query;
pub mod source;
pub mod tab_dispatch;
pub mod wizard;

pub use based_postgres::{PostgresConfig, SslMode, pg_connect_options, postgres_uri, psql_command};

use based_core::ConnectError;
use based_postgres::connect_error;
use sqlx::postgres::{PgConnectOptions, PgPoolOptions};
use sqlx::{Connection, PgPool};
use tokio::time::timeout;

use crate::connection::lifecycle::{Connectable, TestReport};
use crate::connection::tunnel::{open_optional_tunnel, rewrite_tcp_endpoint};
use crate::db::Tokio;
use based_ssh::SshTunnel;

/// Live Postgres connection wrapping a sqlx pool.
pub struct PgConnection {
    /// Held only so the SSH tunnel lives as long as the connection.
    _tunnel: Option<SshTunnel>,
    pub server_version: Option<String>,
    pub pool: PgPool,
}

/// How long one connect attempt may take before it counts as timed out.
const CONNECT_TIMEOUT: Duration = Duration::from_secs(30);

async fn connect_opts(
    config: &PostgresConfig,
) -> Result<(PgConnectOptions, Option<SshTunnel>), ConnectError> {
    let tunnel = open_optional_tunnel(
        config.ssh.as_ref(),
        &config.host,
        config.port,
        config.ssl_mode,
    )
    .await
    .map_err(ConnectError::ssh)?;
    let mut connect = config.clone();
    if let Some(tunnel) = &tunnel {
        rewrite_tcp_endpoint(&mut connect.host, &mut connect.port, tunnel);
    }
    Ok((pg_connect_options(&connect), tunnel))
}

/// One plain connection: the server's version, or why it can't be reached.
/// A pool would retry a refused connection until its acquire timeout and
/// report only "timed out", so the first contact is made without one.
async fn probe(opts: &PgConnectOptions) -> Result<String, ConnectError> {
    let attempt = async {
        let mut conn = sqlx::PgConnection::connect_with(opts).await?;
        let version: String = sqlx::query_scalar("SELECT version()")
            .fetch_one(&mut conn)
            .await?;
        let _ = conn.close().await;
        Ok::<_, sqlx::Error>(version)
    };
    let version = timeout(CONNECT_TIMEOUT, attempt)
        .await
        .map_err(|_| ConnectError::timeout(CONNECT_TIMEOUT))?
        .map_err(connect_error)?;
    Ok(version.lines().next().unwrap_or(&version).to_string())
}

impl Connectable for PgConnection {
    type Config = PostgresConfig;

    fn open(config: Self::Config, cx: &mut App) -> Task<Result<Self, ConnectError>> {
        Tokio::spawn_try(
            cx,
            async move {
                let (opts, tunnel) = connect_opts(&config).await?;
                let version = probe(&opts).await?;
                let pool = PgPoolOptions::new()
                    .max_connections(8)
                    .connect_with(opts)
                    .await
                    .map_err(connect_error)?;
                Ok(Self {
                    _tunnel: tunnel,
                    pool,
                    server_version: Some(version),
                })
            },
            ConnectError::internal,
        )
    }

    fn test(config: &Self::Config, cx: &mut App) -> Task<Result<TestReport, ConnectError>> {
        let config = config.clone();
        Tokio::spawn_try(
            cx,
            async move {
                let start = Instant::now();
                let (opts, _tunnel) = connect_opts(&config).await?;
                let version = probe(&opts).await?;
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
