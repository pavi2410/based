//! Database I/O on Tokio, surfaced to GPUI via [`Tokio`].

use std::future::Future;
use std::thread;

use std::result::Result as StdResult;

use anyhow::Result;
use gpui_kit::{App, AsyncApp};
use sqlx::{PgPool, SqlitePool};
use tokio::task::JoinError;

mod runtime;

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

/// [`run`] for a typed error; `on_panic` turns a crashed task into one.
pub async fn run_try<R: Send + 'static, E: Send + 'static>(
    cx: &mut AsyncApp,
    f: impl Future<Output = StdResult<R, E>> + Send + 'static,
    on_panic: fn(JoinError) -> E,
) -> StdResult<R, E> {
    Tokio::spawn_try(cx, f, on_panic).await
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
