//! GPUI's background / foreground executors are not Tokio. `sqlx` (runtime-tokio)
//! and the `mongodb` crate expect a Tokio context for timers and pool bookkeeping.
//! Bridge all database I/O through [`block_on_db`].

use std::sync::OnceLock;

use tokio::runtime::Runtime;

static DB_RUNTIME: OnceLock<Runtime> = OnceLock::new();

fn db_runtime() -> &'static Runtime {
    DB_RUNTIME.get_or_init(|| {
        tokio::runtime::Builder::new_multi_thread()
            .enable_all()
            .thread_name("based-db")
            .build()
            .expect("Tokio runtime for database I/O")
    })
}

/// Run a future on the shared Tokio runtime. Call only from **non-Tokio** threads
/// (e.g. GPUI `cx.spawn` / `background_executor` worker threads).
pub fn block_on_db<R: Send + 'static>(
    f: impl std::future::Future<Output = R> + Send + 'static,
) -> R {
    db_runtime().block_on(f)
}
