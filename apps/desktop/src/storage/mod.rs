//! SQLite metadata store bootstrap for the desktop app.

use std::path::Path;
use std::sync::Arc;

use crate::db::Tokio;
use anyhow::Result;
use based_storage::MetadataStore;
use gpui_kit::{App, Global};

pub struct AppStorage {
    pub store: Arc<MetadataStore>,
}

impl Global for AppStorage {}

pub fn init(cx: &mut App) -> Result<()> {
    let handle = Tokio::handle(cx);
    let store = handle
        .block_on(MetadataStore::open_default())
        .map_err(|e| anyhow::anyhow!(e))?;
    install(cx, store);
    Ok(())
}

/// Like [`init`] but at an explicit path (isolated stores for tests).
pub fn init_at(cx: &mut App, path: &Path) -> Result<()> {
    let handle = Tokio::handle(cx);
    let store = handle
        .block_on(MetadataStore::open(path))
        .map_err(|e| anyhow::anyhow!(e))?;
    install(cx, store);
    Ok(())
}

fn install(cx: &mut App, store: MetadataStore) {
    cx.set_global(AppStorage {
        store: Arc::new(store),
    });
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
