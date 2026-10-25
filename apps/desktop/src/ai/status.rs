//! Process-wide AI activity for the status bar. Lock-free so a cancelled stream
//! can release its slot from `Drop` without touching the app.

use std::sync::Arc;
use std::sync::atomic::{AtomicU64, AtomicUsize, Ordering};

use based_ai::Usage;
use gpui_kit::{App, Global};

#[derive(Clone, Default)]
pub struct AiStatus {
    active: Arc<AtomicUsize>,
    tokens: Arc<AtomicU64>,
}

impl Global for AiStatus {}

/// Marks one request in flight until dropped (finished or cancelled).
pub struct ActiveGuard(Arc<AtomicUsize>);

impl Drop for ActiveGuard {
    fn drop(&mut self) {
        self.0.fetch_sub(1, Ordering::Relaxed);
    }
}

impl AiStatus {
    pub fn begin(&self) -> ActiveGuard {
        self.active.fetch_add(1, Ordering::Relaxed);
        ActiveGuard(self.active.clone())
    }

    pub fn add_usage(&self, usage: Usage) {
        let total = u64::from(usage.input_tokens) + u64::from(usage.output_tokens);
        self.tokens.fetch_add(total, Ordering::Relaxed);
    }

    pub fn streaming(&self) -> bool {
        self.active.load(Ordering::Relaxed) > 0
    }

    /// Input plus output tokens used since launch.
    pub fn tokens(&self) -> u64 {
        self.tokens.load(Ordering::Relaxed)
    }

    /// Current status, or an idle one before `init`.
    pub fn get(cx: &App) -> Self {
        cx.try_global::<Self>().cloned().unwrap_or_default()
    }
}

pub fn init(cx: &mut App) {
    cx.set_global(AiStatus::default());
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn guard_tracks_in_flight_requests() {
        let s = AiStatus::default();
        assert!(!s.streaming());
        let a = s.begin();
        let b = s.begin();
        drop(a);
        assert!(s.streaming());
        drop(b);
        assert!(!s.streaming());
    }

    #[test]
    fn usage_accumulates() {
        let s = AiStatus::default();
        s.add_usage(Usage {
            input_tokens: 10,
            output_tokens: 5,
        });
        s.add_usage(Usage {
            input_tokens: 1,
            output_tokens: 1,
        });
        assert_eq!(s.tokens(), 17);
    }
}
