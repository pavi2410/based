// redis/ — GPUI panels + connection lifecycle; driver logic in `based-redis`.

pub mod console;
pub mod key_view;
pub mod tab_dispatch;

use std::time::Instant;

use gpui_kit::{App, Task};

pub use based_redis::RedisConfig;
use based_redis::{Conn, connect, ping, redis_cli_command, redis_url, server_version};

use crate::connection::lifecycle::{Connectable, TestReport};
use crate::db::Tokio;

/// Live Redis connection: an auto-reconnecting manager bound to the URL's DB index.
pub struct RedisConnection {
    pub config: RedisConfig,
    pub server_version: Option<String>,
    conn: Conn,
}

impl RedisConnection {
    /// Cheap clone of the shared connection handle.
    pub fn conn(&self) -> Conn {
        self.conn.clone()
    }
}

pub fn redis_uri(config: &RedisConfig, include_password: bool) -> String {
    redis_url(config, include_password)
}

pub fn redis_cli(config: &RedisConfig, include_password: bool) -> String {
    redis_cli_command(config, include_password)
}

impl Connectable for RedisConnection {
    type Config = RedisConfig;

    fn open(config: Self::Config, cx: &mut App) -> Task<anyhow::Result<Self>> {
        Tokio::spawn_result(cx, async move {
            let mut conn = connect(&config.url).await?;
            let server_version = server_version(&mut conn).await;
            Ok(Self {
                config,
                server_version,
                conn,
            })
        })
    }

    fn test(config: &Self::Config, cx: &mut App) -> Task<anyhow::Result<TestReport>> {
        let url = config.url.clone();
        Tokio::spawn_result(cx, async move {
            let start = Instant::now();
            let mut conn = connect(&url).await?;
            ping(&mut conn).await?;
            Ok(TestReport {
                latency_ms: start.elapsed().as_millis() as u64,
                server_version: server_version(&mut conn).await,
                message: Some("ping ok".into()),
            })
        })
    }

    async fn close(self) {
        drop(self.conn);
    }
}
