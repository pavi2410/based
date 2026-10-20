use std::time::Instant;

use gpui_kit::{App, Task};
// mongodb/ — GPUI panels + connection lifecycle; driver logic in `based-mongo`.

pub mod change_stream;
pub mod document_editor;
pub mod document_viewer;
pub mod inspector;
pub mod pipeline_builder;
pub mod tab_dispatch;

pub use based_mongo::{MongoConfig, document_from_json, mongo_uri, mongosh_command};

use mongodb::bson::doc;
use mongodb::{Client, Database};

use based_mongo::{
    apply_auth_source, parse_client_options, resolve_database_name, test_database_name,
};

use crate::connection::lifecycle::{Connectable, TestReport};
use crate::db::Tokio;

/// Live MongoDB connection: client + selected database.
pub struct MongoConnection {
    pub config: MongoConfig,
    pub server_version: Option<String>,
    client: Client,
    database: Database,
}

impl MongoConnection {
    pub fn database(&self) -> &Database {
        &self.database
    }
}

impl Connectable for MongoConnection {
    type Config = MongoConfig;

    fn open(config: Self::Config, cx: &mut App) -> Task<anyhow::Result<Self>> {
        Tokio::spawn_result(cx, async move {
            let mut opts = parse_client_options(&config.uri).await?;
            if let Some(ref src) = config.auth_source {
                apply_auth_source(&mut opts, src);
            }

            let client = Client::with_options(opts.clone())?;
            let db_name = resolve_database_name(&config, &opts);
            let database = client.database(&db_name);

            let server_version = database
                .run_command(doc! { "buildInfo": 1 })
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

    fn test(config: &Self::Config, cx: &mut App) -> Task<anyhow::Result<TestReport>> {
        let config = config.clone();
        Tokio::spawn_result(cx, async move {
            let start = Instant::now();
            let mut opts = parse_client_options(&config.uri).await?;
            if let Some(ref src) = config.auth_source {
                apply_auth_source(&mut opts, src);
            }
            let client = Client::with_options(opts)?;
            let db = test_database_name(&config);
            let database = client.database(db);
            database.run_command(doc! { "ping": 1 }).await?;
            let server_version = database
                .run_command(doc! { "buildInfo": 1 })
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
