//! Persisted session snapshot stored in the metadata SQLite database.

use serde::{Deserialize, Serialize};

use based_storage::{ACTIVE_CONNECTION_ID, ACTIVE_TAB_INDEX, MetadataStore, OPEN_TABS};

use super::spec::TabSpec;

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct SessionSnapshot {
    pub tabs: Vec<TabSpec>,
    pub active: Option<usize>,
    #[serde(default)]
    pub active_connection_id: Option<String>,
    #[serde(default)]
    pub pinned_tabs: Vec<TabSpec>,
}

impl SessionSnapshot {
    pub async fn load(store: &MetadataStore) -> Self {
        let tabs = store
            .get_session_json(OPEN_TABS)
            .await
            .ok()
            .flatten()
            .unwrap_or_default();
        let active = store
            .get_session_json(ACTIVE_TAB_INDEX)
            .await
            .ok()
            .flatten();
        let active_connection_id = store
            .get_session_json(ACTIVE_CONNECTION_ID)
            .await
            .ok()
            .flatten();
        let pinned_tabs = store
            .get_session_json("pinned_tabs")
            .await
            .ok()
            .flatten()
            .unwrap_or_default();
        Self {
            tabs,
            active,
            active_connection_id,
            pinned_tabs,
        }
    }

    pub async fn save(&self, store: &MetadataStore) -> anyhow::Result<()> {
        store.set_session_json(OPEN_TABS, &self.tabs).await?;
        store
            .set_session_json(ACTIVE_TAB_INDEX, &self.active)
            .await?;
        store
            .set_session_json(ACTIVE_CONNECTION_ID, &self.active_connection_id)
            .await?;
        store
            .set_session_json("pinned_tabs", &self.pinned_tabs)
            .await?;
        Ok(())
    }
}
