//! Tokio runtime hosted as a GPUI global. Dropping a spawned GPUI task aborts the Tokio work.

use std::future::Future;
use std::sync::OnceLock;

use anyhow::Result;
use gpui_kit::{App, AppContext, Global, ReadGlobal, Task};
use tokio::runtime::Handle;
use tokio::task::JoinError;

static HANDLE: OnceLock<Handle> = OnceLock::new();

struct AbortOnDrop(tokio::task::AbortHandle);

impl Drop for AbortOnDrop {
    fn drop(&mut self) {
        self.0.abort();
    }
}

struct GlobalTokio {
    owned_runtime: Option<tokio::runtime::Runtime>,
    handle: Handle,
}

impl Global for GlobalTokio {}

impl Drop for GlobalTokio {
    fn drop(&mut self) {
        if let Some(runtime) = self.owned_runtime.take() {
            runtime.shutdown_background();
        }
    }
}

/// Spawns Tokio work and returns a GPUI task.
pub struct Tokio;

impl Tokio {
    pub fn spawn<C, Fut, R>(cx: &C, f: Fut) -> Task<std::result::Result<R, JoinError>>
    where
        C: AppContext,
        Fut: Future<Output = R> + Send + 'static,
        R: Send + 'static,
    {
        cx.read_global(|tokio: &GlobalTokio, cx| {
            let join_handle = tokio.handle.spawn(f);
            let abort = AbortOnDrop(join_handle.abort_handle());
            cx.background_spawn(async move {
                let result = join_handle.await;
                drop(abort);
                result
            })
        })
    }

    pub fn spawn_result<C, Fut, R>(cx: &C, f: Fut) -> Task<Result<R>>
    where
        C: AppContext,
        Fut: Future<Output = Result<R>> + Send + 'static,
        R: Send + 'static,
    {
        cx.read_global(|tokio: &GlobalTokio, cx| {
            let join_handle = tokio.handle.spawn(f);
            let abort = AbortOnDrop(join_handle.abort_handle());
            cx.background_spawn(async move {
                let result = join_handle.await?;
                drop(abort);
                result
            })
        })
    }

    pub fn handle(cx: &App) -> Handle {
        GlobalTokio::global(cx).handle.clone()
    }
}

pub fn init(cx: &mut App) {
    let runtime = tokio::runtime::Builder::new_multi_thread()
        .worker_threads(2)
        .enable_all()
        .build()
        .expect("Failed to initialize Tokio");
    let handle = runtime.handle().clone();
    cx.set_global(GlobalTokio {
        owned_runtime: Some(runtime),
        handle,
    });
    let _ = HANDLE.set(Tokio::handle(cx));
}

pub(super) fn cached_handle() -> Option<Handle> {
    HANDLE.get().cloned()
}
