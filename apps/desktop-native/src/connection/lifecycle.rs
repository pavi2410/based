// Connectable — the ONLY trait crossing engines.
// Covers open / test / close lifecycle ONLY.
// Tab content reaches into engine-specific APIs directly.

use gpui::{App, Task};

#[derive(Debug, Clone)]
pub struct TestReport {
    pub latency_ms: u64,
    pub server_version: Option<String>,
    pub message: Option<String>,
}

pub trait Connectable: 'static + Sized {
    type Config: serde::de::DeserializeOwned + Clone + Send + 'static;

    fn open(config: Self::Config, cx: &mut App) -> Task<anyhow::Result<Self>>;
    fn test(config: &Self::Config, cx: &mut App) -> Task<anyhow::Result<TestReport>>;
    fn close(self) -> impl std::future::Future<Output = ()> + Send + 'static;
}
