// mongodb/ — Fully specialized MongoDB module.
// Nothing from here is shared with postgres/ or sqlite/.
// Implemented in Phase 5.

pub mod change_stream;
pub mod document_editor;
pub mod document_viewer;
pub mod inspector;
pub mod mutations;
pub mod pipeline_builder;
pub mod tree;
pub mod wizard;

use serde::{Deserialize, Serialize};

use crate::connection::lifecycle::{Connectable, TestReport};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MongoConfig {
    pub label: String,
    pub uri: String,
    pub database: Option<String>,
    pub auth_source: Option<String>,
}

/// Live MongoDB connection wrapping a mongodb::Client.
pub struct MongoConnection {
    pub config: MongoConfig,
    // client: mongodb::Client — added in Phase 5
}

impl Connectable for MongoConnection {
    type Config = MongoConfig;

    fn open(_config: Self::Config, _cx: &mut gpui::App) -> gpui::Task<anyhow::Result<Self>> {
        // TODO Phase 5
        gpui::Task::ready(Err(anyhow::anyhow!("MongoDB engine not yet implemented")))
    }

    fn test(_config: &Self::Config, _cx: &mut gpui::App) -> gpui::Task<anyhow::Result<TestReport>> {
        // TODO Phase 5
        gpui::Task::ready(Err(anyhow::anyhow!("MongoDB engine not yet implemented")))
    }

    async fn close(self) {
        // TODO Phase 5
    }
}
