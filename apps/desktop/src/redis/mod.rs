// redis/ — GPUI panels + connection lifecycle; driver logic in `based-redis`.

pub mod console;
pub mod key_view;
pub mod tab_dispatch;

use std::time::Instant;

use gpui_kit::{App, Task};

pub use based_redis::RedisConfig;
use based_redis::{Conn, connect, ping, redis_cli_command, redis_url, server_version};

use crate::connection::descriptor::EngineDescriptor;
use crate::connection::lifecycle::{Connectable, TestReport};
use crate::db::Tokio;
use based_core::EngineKind;

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

/// Engine descriptor for Redis — registered at startup via [`crate::connection::EngineRegistry`].
pub struct RedisEngine;

impl EngineDescriptor for RedisEngine {
    fn kind(&self) -> EngineKind {
        EngineKind::Redis
    }
    fn display_name(&self) -> &str {
        "Redis"
    }
    fn icon_name(&self) -> &str {
        "redis"
    }
    fn default_port(&self) -> Option<u16> {
        Some(6379)
    }
    fn supports_tab_kind(&self, kind: &str) -> bool {
        matches!(kind, "query_editor" | "data_viewer" | "dashboard")
    }
    fn connect_hint(&self) -> &'static str {
        "Connection URL"
    }
}
