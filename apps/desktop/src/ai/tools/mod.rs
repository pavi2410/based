//! Read-only tool backends over the app's live connections.

mod mongo;
mod redis;
mod sql;

use std::sync::Arc;

use std::fmt::Display;

use based_ai::tools::ToolBackend;
use based_ai::{AiError, AiErrorKind};
use gpui_kit::App;

use crate::connection::{AnyConnection, ConnectionEntry, ConnectionState};

fn fail(e: impl Display) -> AiError {
    AiError::new(AiErrorKind::Other, format!("{e:#}"))
}

/// Backend for a connected `entry`; `None` while it is not connected.
pub fn backend(entry: &ConnectionEntry, cx: &App) -> Option<Arc<dyn ToolBackend>> {
    let ConnectionState::Connected(conn) = &entry.state else {
        return None;
    };
    Some(match conn {
        AnyConnection::Postgres(c) => Arc::new(sql::SqlTools(sql::SqlPool::Postgres(
            c.read(cx).pool.clone(),
        ))),
        AnyConnection::SQLite(c) => {
            Arc::new(sql::SqlTools(sql::SqlPool::Sqlite(c.read(cx).pool.clone())))
        }
        AnyConnection::MongoDB(c) => Arc::new(mongo::MongoTools(c.read(cx).database().clone())),
        AnyConnection::Redis(c) => Arc::new(redis::RedisTools(c.read(cx).conn())),
    })
}
