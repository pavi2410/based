// Connectable — the ONLY trait crossing engines.
// Covers open / test / close lifecycle ONLY.
// Tab content reaches into engine-specific APIs directly.

use gpui::{App, Task};

pub struct TestReport {
    pub ok: bool,
    pub latency_ms: u64,
    pub message: String,
}

pub trait Connectable: 'static + Sized {
    type Config: serde::de::DeserializeOwned + Clone + Send + 'static;

    fn open(config: Self::Config, cx: &mut App) -> Task<anyhow::Result<Self>>;
    fn test(config: &Self::Config, cx: &mut App) -> Task<anyhow::Result<TestReport>>;
    fn close(self) -> impl std::future::Future<Output = ()> + Send + 'static;
}
