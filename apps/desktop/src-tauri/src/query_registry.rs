//! In-flight query registry.
//!
//! Scaffolding for the upcoming "cancel" button on the query editor.
//! Stores a cancellation flag per in-flight query, keyed by a token
//! the frontend holds on to. The actual wiring into `execute_raw_sql`
//! / `execute_raw_mongo` lands with the Phase 2 "params + history +
//! cancel" work; this module keeps the concern isolated so that
//! feature only has to:
//!
//!   1. call `registry.register()` before starting a query,
//!   2. poll `handle.is_cancelled()` or race the query future against
//!      `handle.cancelled().await` in a `tokio::select!`,
//!   3. call `registry.finish(token)` when the query resolves.
//!
//! No external deps: a single `Arc<AtomicBool>` + a `Notify` is enough
//! to express both "is this cancelled?" and "wake me when it is".
//!
//! Cancellation semantics: cooperative. Dropping the query future on
//! the server side releases any engine resources the driver holds; we
//! don't try to inject a SIGINT into a running Postgres backend. The
//! UX target is "feels responsive" not "forcefully kill the query on
//! the wire" — that's a separate feature.

use std::collections::HashMap;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};

use serde::{Deserialize, Serialize};
use specta::Type;
use tokio::sync::{Notify, RwLock};
use uuid::Uuid;

/// Opaque token the frontend receives and echoes back to cancel a
/// specific in-flight query. Serialised as a plain string so it can
/// round-trip through IPC without pulling `Uuid` into the binding.
#[derive(Debug, Clone, Serialize, Deserialize, Type)]
pub struct QueryToken(pub String);

impl QueryToken {
    fn new() -> Self {
        Self(Uuid::new_v4().to_string())
    }
}

/// Handle held by the query executor. `is_cancelled()` is the fast
/// poll-path; `cancelled()` is an awaitable future for `tokio::select!`.
#[derive(Clone)]
pub struct CancellationHandle {
    flag: Arc<AtomicBool>,
    notify: Arc<Notify>,
}

#[allow(dead_code)]
impl CancellationHandle {
    pub fn is_cancelled(&self) -> bool {
        self.flag.load(Ordering::SeqCst)
    }

    /// Await cancellation. Safe to call even after cancellation — it
    /// returns immediately in that case so `tokio::select!` branches
    /// stay correct in the racy "cancelled just before we await" path.
    pub async fn cancelled(&self) {
        if self.flag.load(Ordering::SeqCst) {
            return;
        }
        loop {
            let notified = self.notify.notified();
            if self.flag.load(Ordering::SeqCst) {
                return;
            }
            notified.await;
            if self.flag.load(Ordering::SeqCst) {
                return;
            }
        }
    }

    fn cancel(&self) {
        self.flag.store(true, Ordering::SeqCst);
        self.notify.notify_waiters();
    }
}

#[derive(Default)]
pub struct QueryRegistry {
    inner: RwLock<HashMap<String, CancellationHandle>>,
}

#[allow(dead_code)]
impl QueryRegistry {
    pub fn new() -> Self {
        Self::default()
    }

    /// Register a new in-flight query. Returns the token (to expose
    /// to the UI) and the handle (to check cancellation mid-query).
    pub async fn register(&self) -> (QueryToken, CancellationHandle) {
        let token = QueryToken::new();
        let handle = self.register_with(token.0.clone()).await;
        (token, handle)
    }

    /// Register a query using a caller-supplied token id (typically
    /// the UUID the frontend already generated before invoking
    /// `execute_raw_*`). This is the path executors actually use so
    /// the frontend can race a `cancel_query(token)` against an
    /// in-flight execute without first doing a registration
    /// round-trip.
    pub async fn register_with(&self, token: String) -> CancellationHandle {
        let handle = CancellationHandle {
            flag: Arc::new(AtomicBool::new(false)),
            notify: Arc::new(Notify::new()),
        };
        self.inner.write().await.insert(token, handle.clone());
        handle
    }

    /// Cancel an in-flight query by token. No-op if the token is
    /// already finished/unknown — the frontend can race a stale
    /// cancel with a fast query resolution without breaking.
    pub async fn cancel(&self, token: &str) -> bool {
        let guard = self.inner.read().await;
        if let Some(handle) = guard.get(token) {
            handle.cancel();
            true
        } else {
            false
        }
    }

    /// Remove the registry entry after the query finishes. Called
    /// whether the query succeeded, errored, or was cancelled.
    pub async fn finish(&self, token: &QueryToken) {
        self.inner.write().await.remove(&token.0);
    }

    /// Same as `finish` but accepts a raw string id, matching the
    /// shape used by `register_with`.
    pub async fn finish_by_id(&self, token: &str) {
        self.inner.write().await.remove(token);
    }

    /// Count of active queries. Used by the status bar & tests.
    pub async fn len(&self) -> usize {
        self.inner.read().await.len()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn cancel_flips_flag_and_wakes_await() {
        let reg = QueryRegistry::new();
        let (token, handle) = reg.register().await;
        assert!(!handle.is_cancelled());

        let waiter = {
            let h = handle.clone();
            tokio::spawn(async move {
                h.cancelled().await;
            })
        };

        assert!(reg.cancel(&token.0).await);
        tokio::time::timeout(std::time::Duration::from_millis(500), waiter)
            .await
            .expect("cancelled() should wake")
            .expect("task must not panic");
        assert!(handle.is_cancelled());
    }

    #[tokio::test]
    async fn cancel_is_idempotent_and_unknown_token_is_false() {
        let reg = QueryRegistry::new();
        assert!(!reg.cancel("nonexistent").await);

        let (token, _handle) = reg.register().await;
        assert!(reg.cancel(&token.0).await);
        reg.finish(&token).await;
        assert!(!reg.cancel(&token.0).await);
    }

    #[tokio::test]
    async fn finish_drops_entry() {
        let reg = QueryRegistry::new();
        let (token, _) = reg.register().await;
        assert_eq!(reg.len().await, 1);
        reg.finish(&token).await;
        assert_eq!(reg.len().await, 0);
    }
}
