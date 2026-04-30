//! Database I/O runs on Tokio (`sqlx`, `mongodb`). GPUI executors are not Tokio, so we use
//! [gpui_tokio](https://github.com/zed-industries/zed/blob/main/crates/gpui_tokio/src/gpui_tokio.rs):
//! `gpui_tokio::init` + `Tokio::spawn` / `spawn_result` bridge Tokio futures to GPUI tasks
//! without blocking the UI thread the way `Runtime::block_on` from a GPUI task does.

use std::sync::OnceLock;

use anyhow::Result;
use gpui::App;
use gpui::AsyncApp;
use gpui_tokio::Tokio;
use sqlx::{PgPool, SqlitePool};

static HANDLE: OnceLock<tokio::runtime::Handle> = OnceLock::new();

/// Register Tokio with GPUI and cache the handle for pool shutdown. Call once from `App::run`
/// (after `gpui_component::init`, before opening windows).
pub fn init(cx: &mut App) {
    gpui_tokio::init(cx);
    let _ = HANDLE.set(Tokio::handle(cx));
}

/// Run an fallible async closure on Tokio; await from `cx.spawn(async |_, cx| { ... })` where `cx` is `&mut AsyncApp`.
pub async fn run<R: Send + 'static>(
    cx: &mut AsyncApp,
    f: impl std::future::Future<Output = Result<R>> + Send + 'static,
) -> Result<R> {
    Tokio::spawn_result(cx, f).await
}

/// Run an infallible async closure on Tokio; `JoinError` is mapped to `anyhow::Error`.
pub async fn run_infallible<R: Send + 'static>(
    cx: &mut AsyncApp,
    f: impl std::future::Future<Output = R> + Send + 'static,
) -> Result<R> {
    Tokio::spawn(cx, f)
        .await
        .map_err(|e| anyhow::anyhow!(e))
}

pub fn close_sqlite_pool(pool: SqlitePool) {
    if let Some(h) = HANDLE.get() {
        let h = h.clone();
        std::thread::spawn(move || {
            let _ = h.block_on(async move { pool.close().await });
        });
    }
}

pub fn close_pg_pool(pool: PgPool) {
    if let Some(h) = HANDLE.get() {
        let h = h.clone();
        std::thread::spawn(move || {
            let _ = h.block_on(async move { pool.close().await });
        });
    }
}
