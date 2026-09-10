//! Database I/O on Tokio, surfaced to GPUI via [`Tokio`].

use std::future::Future;
use std::thread;

use anyhow::Result;
use gpui_kit::{App, AsyncApp};
use sqlx::{PgPool, SqlitePool};

mod runtime;

pub mod column_catalog;
pub use runtime::Tokio;

/// Register Tokio with GPUI. Call once from `App::run` after `gpui_kit::init`.
pub fn init(cx: &mut App) {
    runtime::init(cx);
}

/// Run a fallible async closure on Tokio; await from `cx.spawn(async |_, cx| { ... })`.
pub async fn run<R: Send + 'static>(
    cx: &mut AsyncApp,
    f: impl Future<Output = Result<R>> + Send + 'static,
) -> Result<R> {
    Tokio::spawn_result(cx, f).await
}

/// Run an infallible async closure on Tokio; `JoinError` is mapped to `anyhow::Error`.
pub async fn run_infallible<R: Send + 'static>(
    cx: &mut AsyncApp,
    f: impl Future<Output = R> + Send + 'static,
) -> Result<R> {
    Tokio::spawn(cx, f).await.map_err(|e| anyhow::anyhow!(e))
}

pub fn close_sqlite_pool(pool: SqlitePool) {
    if let Some(h) = runtime::cached_handle() {
        thread::spawn(move || {
            h.block_on(async move { pool.close().await });
        });
    }
}

pub fn close_pg_pool(pool: PgPool) {
    if let Some(h) = runtime::cached_handle() {
        thread::spawn(move || {
            h.block_on(async move { pool.close().await });
        });
    }
}
