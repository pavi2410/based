// mongodb/ — GPUI panels + connection lifecycle; driver logic in `based-mongo`.

pub mod change_stream;
pub mod document_editor;
pub mod document_viewer;
pub mod inspector;
pub mod pipeline_builder;
pub mod tree;
pub mod wizard;

pub use based_mongo::{MongoConfig, document_from_json};

use mongodb::Database;
use mongodb::bson::doc;
use mongodb::options::ClientOptions;

use based_mongo::{apply_auth_source, resolve_database_name, test_database_name};

use crate::connection::lifecycle::{Connectable, TestReport};
use gpui_tokio::Tokio;

/// Live MongoDB connection: client + selected database.
pub struct MongoConnection {
    pub config: MongoConfig,
    pub server_version: Option<String>,
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

impl Connectable for MongoConnection {
    type Config = MongoConfig;

    fn open(config: Self::Config, cx: &mut gpui::App) -> gpui::Task<anyhow::Result<Self>> {
        Tokio::spawn_result(cx, async move {
            let mut opts = ClientOptions::parse(&config.uri).await?;
            if let Some(ref src) = config.auth_source {
                apply_auth_source(&mut opts, src);
            }

            let client = mongodb::Client::with_options(opts.clone())?;
            let db_name = resolve_database_name(&config, &opts);
            let database = client.database(&db_name);

            let server_version = database
                .run_command(doc! { "buildInfo": 1 }, None)
                .await
                .ok()
                .and_then(|info| info.get_str("version").ok().map(ToString::to_string));

            Ok(Self {
                config,
                server_version,
                client,
                database,
            })
        })
    }

    fn test(config: &Self::Config, cx: &mut gpui::App) -> gpui::Task<anyhow::Result<TestReport>> {
        let config = config.clone();
        Tokio::spawn_result(cx, async move {
            let start = std::time::Instant::now();
            let mut opts = ClientOptions::parse(&config.uri).await?;
            if let Some(ref src) = config.auth_source {
                apply_auth_source(&mut opts, src);
            }
            let client = mongodb::Client::with_options(opts)?;
            let db = test_database_name(&config);
            let database = client.database(db);
            database
                .run_command(mongodb::bson::doc! { "ping": 1 }, None)
                .await?;
            let server_version = database
                .run_command(doc! { "buildInfo": 1 }, None)
                .await
                .ok()
                .and_then(|info| info.get_str("version").ok().map(ToString::to_string));
            Ok(TestReport {
                latency_ms: start.elapsed().as_millis() as u64,
                server_version,
                message: Some("ping ok".into()),
            })
        })
    }

    async fn close(self) {
        drop(self.client);
    }
}

use crate::connection::descriptor::EngineDescriptor;
use based_core::EngineKind;

/// Engine descriptor for MongoDB — registered at startup via [`crate::connection::EngineRegistry`].
pub struct MongoEngine;

impl EngineDescriptor for MongoEngine {
    fn kind(&self) -> EngineKind {
        EngineKind::MongoDB
    }
    fn display_name(&self) -> &str {
        "MongoDB"
    }
    fn icon_name(&self) -> &str {
        "mongodb"
    }
    fn default_port(&self) -> Option<u16> {
        Some(27017)
    }
    fn supports_tab_kind(&self, kind: &str) -> bool {
        matches!(
            kind,
            "query_editor"
                | "pipeline"
                | "data_viewer"
                | "inspector"
                | "document_insert"
                | "dashboard"
        )
    }
}
