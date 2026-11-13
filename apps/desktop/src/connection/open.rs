use based_core::ConnectError;

use crate::db::Tokio;
use gpui_kit::{App, AppContext, Task};

use super::lifecycle::Connectable;
use super::{AnyConnection, ConnectionConfig};
use crate::mongodb::MongoConnection;
use crate::postgres::PgConnection;
use crate::redis::RedisConnection;
use crate::sqlite::SqliteConnection;

/// Plain connection value from `Connectable::open` before GPUI entity wrapping.
pub enum OpenedConnection {
    Sqlite(SqliteConnection),
    Postgres(PgConnection),
    MongoDB(MongoConnection),
    Redis(RedisConnection),
}

/// Start open using the right `Connectable` impl (single dispatch site).
pub fn open_connection(
    config: ConnectionConfig,
    cx: &mut App,
) -> Task<Result<OpenedConnection, ConnectError>> {
    match config {
        ConnectionConfig::SQLite(cfg) => {
            let task = SqliteConnection::open(cfg, cx);
            Tokio::spawn_try(
                cx,
                async move { Ok(OpenedConnection::Sqlite(task.await?)) },
                ConnectError::internal,
            )
        }
        ConnectionConfig::Postgres(cfg) => {
            let task = PgConnection::open(cfg, cx);
            Tokio::spawn_try(
                cx,
                async move { Ok(OpenedConnection::Postgres(task.await?)) },
                ConnectError::internal,
            )
        }
        ConnectionConfig::MongoDB(cfg) => {
            let task = MongoConnection::open(cfg, cx);
            Tokio::spawn_try(
                cx,
                async move { Ok(OpenedConnection::MongoDB(task.await?)) },
                ConnectError::internal,
            )
        }
        ConnectionConfig::Redis(cfg) => {
            let task = RedisConnection::open(cfg, cx);
            Tokio::spawn_try(
                cx,
                async move { Ok(OpenedConnection::Redis(task.await?)) },
                ConnectError::internal,
            )
        }
    }
}

/// Wrap opened handle into registry-ready `AnyConnection`.
pub fn opened_into_any(opened: OpenedConnection, cx: &mut App) -> AnyConnection {
    match opened {
        OpenedConnection::Sqlite(conn) => AnyConnection::SQLite(cx.new(|_| conn)),
        OpenedConnection::Postgres(conn) => AnyConnection::Postgres(cx.new(|_| conn)),
        OpenedConnection::MongoDB(conn) => AnyConnection::MongoDB(cx.new(|_| conn)),
        OpenedConnection::Redis(conn) => AnyConnection::Redis(cx.new(|_| conn)),
    }
}
