//! SQLite metadata store bootstrap for the desktop app.

use std::sync::Arc;

use anyhow::Result;
use based_storage::MetadataStore;
use gpui::{App, Global};
use gpui_tokio::Tokio;

pub struct AppStorage {
    pub store: Arc<MetadataStore>,
}

impl Global for AppStorage {}

pub fn init(cx: &mut App) -> Result<()> {
    let handle = Tokio::handle(cx);
    let store = handle
        .block_on(MetadataStore::open_default())
        .map_err(|e| anyhow::anyhow!(e))?;
    cx.set_global(AppStorage {
        store: Arc::new(store),
    });
    Ok(())
}

pub fn store(cx: &App) -> Arc<MetadataStore> {
    cx.try_global::<AppStorage>()
        .expect("AppStorage not initialized — storage::init must be called before storage::store")
        .store
        .clone()
}

pub fn try_store(cx: &App) -> Option<Arc<MetadataStore>> {
    cx.try_global::<AppStorage>().map(|s| s.store.clone())
}
