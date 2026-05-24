use gpui::{App, AppContext, Task};
use gpui_tokio::Tokio;

use crate::connection::lifecycle::Connectable;
use crate::connection::{AnyConnection, ConnectionConfig};
use crate::mongodb::MongoConnection;
use crate::postgres::PgConnection;
use crate::sqlite::SqliteConnection;

/// Plain connection value from `Connectable::open` before GPUI entity wrapping.
pub enum OpenedConnection {
    Sqlite(SqliteConnection),
    Postgres(PgConnection),
    MongoDB(MongoConnection),
}

/// Start open using the right `Connectable` impl (single dispatch site).
pub fn open_connection(
    config: ConnectionConfig,
    cx: &mut App,
) -> Task<anyhow::Result<OpenedConnection>> {
    match config {
        ConnectionConfig::SQLite(cfg) => {
            let task = SqliteConnection::open(cfg, cx);
            Tokio::spawn_result(cx, async move { Ok(OpenedConnection::Sqlite(task.await?)) })
        }
        ConnectionConfig::Postgres(cfg) => {
            let task = PgConnection::open(cfg, cx);
            Tokio::spawn_result(
                cx,
                async move { Ok(OpenedConnection::Postgres(task.await?)) },
            )
        }
        ConnectionConfig::MongoDB(cfg) => {
            let task = MongoConnection::open(cfg, cx);
            Tokio::spawn_result(
                cx,
                async move { Ok(OpenedConnection::MongoDB(task.await?)) },
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
    }
}
