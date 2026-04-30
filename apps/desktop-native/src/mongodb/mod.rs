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

use mongodb::options::{ClientOptions, Credential};
use mongodb::Database;

use crate::connection::lifecycle::{Connectable, TestReport};
use crate::tokio_bridge;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MongoConfig {
    pub label: String,
    pub uri: String,
    pub database: Option<String>,
    pub auth_source: Option<String>,
}

/// Live MongoDB connection: client + selected database.
pub struct MongoConnection {
    pub config: MongoConfig,
    client: mongodb::Client,
    database: Database,
}

impl MongoConnection {
    pub fn database(&self) -> &Database {
        &self.database
    }

    pub fn client(&self) -> &mongodb::Client {
        &self.client
    }
}

fn apply_auth_source(opts: &mut ClientOptions, src: &str) {
    match opts.credential.as_mut() {
        Some(cred) => {
            cred.source = Some(src.to_string());
        }
        None => {
            opts.credential = Some(
                Credential::builder()
                    .source(Some(src.to_string()))
                    .build(),
            );
        }
    }
}

impl Connectable for MongoConnection {
    type Config = MongoConfig;

    fn open(config: Self::Config, cx: &mut gpui::App) -> gpui::Task<anyhow::Result<Self>> {
        cx.background_executor().spawn(async move {
            tokio_bridge::block_on_db(async move {
                let mut opts = ClientOptions::parse(&config.uri).await?;
                if let Some(ref src) = config.auth_source {
                    apply_auth_source(&mut opts, src);
                }

                let client = mongodb::Client::with_options(opts.clone())?;
                let db_name = config
                    .database
                    .clone()
                    .or_else(|| opts.default_database.as_ref().map(|s| s.to_string()))
                    .unwrap_or_else(|| "test".to_string());
                let database = client.database(&db_name);

                Ok(Self {
                    config,
                    client,
                    database,
                })
            })
        })
    }

    fn test(config: &Self::Config, cx: &mut gpui::App) -> gpui::Task<anyhow::Result<TestReport>> {
        let config = config.clone();
        cx.background_executor().spawn(async move {
            tokio_bridge::block_on_db(async move {
                let start = std::time::Instant::now();
                let mut opts = ClientOptions::parse(&config.uri).await?;
                if let Some(ref src) = config.auth_source {
                    apply_auth_source(&mut opts, src);
                }
                let client = mongodb::Client::with_options(opts)?;
                let db = config.database.as_deref().unwrap_or("admin");
                client
                    .database(db)
                    .run_command(mongodb::bson::doc! { "ping": 1 }, None)
                    .await?;
                Ok(TestReport {
                    latency_ms: start.elapsed().as_millis() as u64,
                    server_version: None,
                    message: Some("ping ok".into()),
                })
            })
        })
    }

    async fn close(self) {
        drop(self.client);
    }
}
