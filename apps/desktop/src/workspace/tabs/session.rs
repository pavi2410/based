//! Persisted session snapshot stored in the metadata SQLite database.

use serde::{Deserialize, Serialize};

use std::mem;

use based_storage::{ACTIVE_CONNECTION_ID, ACTIVE_TAB_INDEX, MetadataStore, OPEN_TABS};

use super::manager::TabManager;
use super::spec::TabSpec;
use crate::connection::ConnectionId;

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct SessionSnapshot {
    pub tabs: Vec<TabSpec>,
    pub active: Option<usize>,
    #[serde(default)]
    pub active_connection_id: Option<String>,
    #[serde(default)]
    pub pinned_tabs: Vec<TabSpec>,
}

/// Load tabs from the session, skipping any that fail to deserialize.
/// This gracefully handles the migration from pre-QueryEditorInit sessions
/// (P3 refactor), where QueryEditor had flat serde fields.
async fn load_tabs_with_migration(store: &MetadataStore) -> Vec<TabSpec> {
    if let Ok(Some(tabs)) = store.get_session_json::<Vec<TabSpec>>(OPEN_TABS).await {
        return tabs;
    }
    if let Ok(Some(raw)) = store
        .get_session_json::<Vec<serde_json::Value>>(OPEN_TABS)
        .await
    {
        return raw
            .into_iter()
            .filter_map(|v| serde_json::from_value(v).ok())
            .collect();
    }
    vec![]
}

impl SessionSnapshot {
    pub async fn load(store: &MetadataStore) -> Self {
        let tabs = load_tabs_with_migration(store).await;
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

impl SessionSnapshot {
    /// Snapshot of the open tabs; `pending` are restored tabs whose connection is still down.
    /// `None` when there is nothing worth saving.
    pub fn capture(
        tm: &TabManager,
        pending: &[TabSpec],
        active_connection_id: Option<String>,
    ) -> Option<Self> {
        let mut tabs = Vec::new();
        let mut active = None;
        for (i, t) in tm.tabs.iter().enumerate() {
            if !t.spec.persist_in_session() {
                continue;
            }
            if tm.active_idx == Some(i) {
                active = Some(tabs.len());
            }
            tabs.push(t.spec.clone());
        }
        let tabs = merge_live_and_pending(tabs, pending);
        if tabs.is_empty() {
            return None;
        }
        Some(Self {
            tabs,
            active,
            active_connection_id,
            pinned_tabs: tm
                .pinned_specs()
                .into_iter()
                .filter(TabSpec::persist_in_session)
                .collect(),
        })
    }
}

/// Restore bookkeeping and the coalescing write queue for one workspace.
#[derive(Default)]
pub struct SessionState {
    restored: bool,
    /// Saved tabs waiting for their connection. Kept until the user closes them.
    pending: Vec<TabSpec>,
    /// Pending specs already dispatched this run, so edits do not spawn duplicates.
    opened: Vec<TabSpec>,
    /// Newest snapshot waiting for the writer (newer replaces older).
    queued: Option<SessionSnapshot>,
    writing: bool,
}

impl SessionState {
    /// `true` exactly once: the first call, when the saved session should be restored.
    pub fn begin_restore(&mut self) -> bool {
        !mem::replace(&mut self.restored, true)
    }

    /// Tabs to bring back (Home and non-persistent specs are skipped).
    pub fn set_pending(&mut self, tabs: impl IntoIterator<Item = TabSpec>) {
        self.pending = tabs
            .into_iter()
            .filter(|s| !matches!(s, TabSpec::Home) && s.persist_in_session())
            .collect();
    }

    pub fn pending(&self) -> &[TabSpec] {
        &self.pending
    }

    /// Pending specs that have not been dispatched yet.
    pub fn to_open(&self) -> Vec<TabSpec> {
        self.pending
            .iter()
            .filter(|s| !self.opened.contains(s))
            .cloned()
            .collect()
    }

    pub fn mark_opened(&mut self, spec: TabSpec) {
        self.opened.push(spec);
    }

    /// The user closed this tab: stop restoring and re-saving it.
    pub fn forget(&mut self, spec: &TabSpec) {
        self.pending.retain(|s| s != spec);
        self.opened.retain(|s| s != spec);
    }

    /// Whether a restored tab for `conn` is still pending (so its tabs must not be reset).
    pub fn awaits(&self, conn: &ConnectionId) -> bool {
        self.pending.iter().any(|s| s.conn_id() == Some(conn))
    }

    /// Queue a snapshot. Returns `true` when the caller must start the writer.
    pub fn enqueue(&mut self, snapshot: SessionSnapshot) -> bool {
        self.queued = Some(snapshot);
        !mem::replace(&mut self.writing, true)
    }

    pub fn take_queued(&mut self) -> Option<SessionSnapshot> {
        self.queued.take()
    }

    pub fn finish_writing(&mut self) {
        self.writing = false;
    }
}

/// Live dock tabs plus session tabs that have not opened yet (connection still down).
pub fn merge_live_and_pending(live: Vec<TabSpec>, pending: &[TabSpec]) -> Vec<TabSpec> {
    let mut tabs = live;
    for spec in pending {
        if spec.persist_in_session() && !tabs.iter().any(|open| open == spec) {
            tabs.push(spec.clone());
        }
    }
    tabs
}

#[cfg(test)]
mod tests {
    use std::slice;

    use super::*;
    use crate::connection::ConnectionId;
    use crate::workspace::tabs::spec::QueryEditorInit;

    #[test]
    fn pending_session_tabs_survive_home_only_live_set() {
        let live = vec![TabSpec::Home];
        let pending = vec![
            TabSpec::Dashboard {
                conn_id: ConnectionId("events".into()),
            },
            TabSpec::DataViewer {
                conn_id: ConnectionId("events".into()),
                object: "events".into(),
                filter: None,
            },
        ];
        let tabs = merge_live_and_pending(live, &pending);
        assert_eq!(tabs.len(), 3);
        assert!(matches!(tabs[0], TabSpec::Home));
        assert!(matches!(tabs[1], TabSpec::Dashboard { .. }));
        assert!(matches!(tabs[2], TabSpec::DataViewer { .. }));
    }

    #[test]
    fn dashboard_plus_blank_query_does_not_drop_restored_sql_or_viewer() {
        let conn = ConnectionId("events".into());
        let live = vec![
            TabSpec::Dashboard {
                conn_id: conn.clone(),
            },
            TabSpec::blank_query_editor(conn.clone()),
        ];
        let pending = vec![
            TabSpec::Dashboard {
                conn_id: conn.clone(),
            },
            TabSpec::QueryEditor {
                conn_id: conn.clone(),
                init: QueryEditorInit::Sql {
                    sql: Some("SELECT * FROM txns".into()),
                    auto_run: false,
                },
            },
            TabSpec::DataViewer {
                conn_id: conn,
                object: "txns".into(),
                filter: None,
            },
        ];
        let tabs = merge_live_and_pending(live, &pending);
        assert!(
            tabs.iter().any(|t| matches!(
                t,
                TabSpec::QueryEditor {
                    init: QueryEditorInit::Sql {
                        sql: Some(sql),
                        ..
                    },
                    ..
                } if sql == "SELECT * FROM txns"
            )),
            "saved SQL must survive a live blank query: {tabs:?}"
        );
        assert!(
            tabs.iter().any(|t| matches!(
                t,
                TabSpec::DataViewer { object, .. } if object == "txns"
            )),
            "data viewer must survive dashboard+blank snapshot: {tabs:?}"
        );
    }

    #[test]
    fn restore_runs_once() {
        let mut st = SessionState::default();
        assert!(st.begin_restore());
        assert!(!st.begin_restore());
    }

    #[test]
    fn pending_tabs_open_once_until_forgotten() {
        let mut st = SessionState::default();
        let viewer = TabSpec::DataViewer {
            conn_id: ConnectionId("c".into()),
            object: "t".into(),
            filter: None,
        };
        st.set_pending([TabSpec::Home, viewer.clone()]);
        assert_eq!(
            st.pending(),
            slice::from_ref(&viewer),
            "Home is never restored"
        );
        assert_eq!(st.to_open(), slice::from_ref(&viewer));
        st.mark_opened(viewer.clone());
        assert!(st.to_open().is_empty());
        assert!(st.awaits(&ConnectionId("c".into())));
        st.forget(&viewer);
        assert!(st.pending().is_empty());
        assert!(!st.awaits(&ConnectionId("c".into())));
    }

    #[test]
    fn writer_starts_once_and_keeps_only_the_newest_snapshot() {
        let mut st = SessionState::default();
        let snap = |n: usize| SessionSnapshot {
            active: Some(n),
            ..Default::default()
        };
        assert!(st.enqueue(snap(1)), "first enqueue starts the writer");
        assert!(!st.enqueue(snap(2)), "writer already running");
        assert_eq!(st.take_queued().unwrap().active, Some(2));
        assert!(st.take_queued().is_none());
        st.finish_writing();
        assert!(st.enqueue(snap(3)));
    }
}
