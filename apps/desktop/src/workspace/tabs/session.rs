//! Persisted session snapshot stored in the metadata SQLite database.

use serde::{Deserialize, Serialize};

use std::mem;

use based_storage::{ACTIVE_CONNECTION_ID, ACTIVE_TAB_INDEX, MetadataStore, OPEN_TABS};

use super::manager::{TabId, TabManager};
use super::spec::TabSpec;
use crate::connection::ConnectionId;

/// Older builds kept pins as a separate list of specs; read once, then cleared.
const LEGACY_PINNED_TABS: &str = "pinned_tabs";

/// One saved tab: what it shows, and whether it is pinned.
///
/// Serialized flat, so an entry written before pins moved here (a bare
/// `TabSpec`) reads as an unpinned tab.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SessionTab {
    #[serde(flatten)]
    pub spec: TabSpec,
    #[serde(default, skip_serializing_if = "is_false")]
    pub pinned: bool,
}

fn is_false(v: &bool) -> bool {
    !*v
}

impl SessionTab {
    pub fn new(spec: TabSpec) -> Self {
        Self {
            spec,
            pinned: false,
        }
    }
}

#[derive(Debug, Clone, Default)]
pub struct SessionSnapshot {
    pub tabs: Vec<SessionTab>,
    pub active: Option<usize>,
    pub active_connection_id: Option<ConnectionId>,
}

/// Load tabs from the session, skipping any that fail to deserialize.
/// This gracefully handles the migration from pre-QueryEditorInit sessions
/// (P3 refactor), where QueryEditor had flat serde fields.
async fn load_tabs_with_migration(store: &MetadataStore) -> Vec<SessionTab> {
    if let Ok(Some(tabs)) = store.get_session_json::<Vec<SessionTab>>(OPEN_TABS).await {
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

/// Mark the first unpinned tab equal to each legacy pinned spec as pinned.
fn apply_legacy_pins(tabs: &mut [SessionTab], pinned: &[TabSpec]) {
    for spec in pinned {
        if let Some(tab) = tabs.iter_mut().find(|t| !t.pinned && t.spec == *spec) {
            tab.pinned = true;
        }
    }
}

impl SessionSnapshot {
    pub async fn load(store: &MetadataStore) -> Self {
        let mut tabs = load_tabs_with_migration(store).await;
        let legacy_pins: Vec<TabSpec> = store
            .get_session_json(LEGACY_PINNED_TABS)
            .await
            .ok()
            .flatten()
            .unwrap_or_default();
        apply_legacy_pins(&mut tabs, &legacy_pins);
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
        Self {
            tabs,
            active,
            active_connection_id,
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
        // Pins live on the tabs now; an old list would re-pin by equality.
        store
            .set_session_json(LEGACY_PINNED_TABS, &Vec::<TabSpec>::new())
            .await?;
        Ok(())
    }

    /// Snapshot of the open tabs plus restored tabs that are not live (their
    /// connection is down, or the center was replaced). `None` when there is
    /// nothing worth saving.
    pub fn capture(
        tm: &TabManager,
        session: &SessionState,
        active_connection_id: Option<ConnectionId>,
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
            tabs.push(SessionTab {
                spec: t.spec.clone(),
                pinned: t.pinned,
            });
        }
        tabs.extend(session.not_live(tm).cloned());
        if tabs.is_empty() {
            return None;
        }
        Some(Self {
            tabs,
            active,
            active_connection_id,
        })
    }
}

/// A restored tab and, once it opened this run, the live tab that stands for it.
#[derive(Debug, Clone)]
struct PendingTab {
    tab: SessionTab,
    live: Option<TabId>,
}

/// Restore bookkeeping and the coalescing write queue for one workspace.
#[derive(Default)]
pub struct SessionState {
    restored: bool,
    /// Saved tabs, kept until the user closes them: opening a connection can
    /// replace the center, and these bring the tabs back next launch.
    pending: Vec<PendingTab>,
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
    pub fn set_pending(&mut self, tabs: impl IntoIterator<Item = SessionTab>) {
        self.pending = tabs
            .into_iter()
            .filter(|t| !matches!(t.spec, TabSpec::Home) && t.spec.persist_in_session())
            .map(|tab| PendingTab { tab, live: None })
            .collect();
    }

    /// Restored tabs not dispatched yet this run, by position.
    pub fn to_open(&self) -> Vec<(usize, SessionTab)> {
        self.pending
            .iter()
            .enumerate()
            .filter(|(_, p)| p.live.is_none())
            .map(|(i, p)| (i, p.tab.clone()))
            .collect()
    }

    /// The restored tab at `index` opened as `live`; the live tab is saved in its place.
    pub fn link(&mut self, index: usize, live: TabId) {
        if let Some(p) = self.pending.get_mut(index) {
            p.live = Some(live);
        }
    }

    /// The restored tab at `index` can never open (its engine has no such tab).
    pub fn drop_pending(&mut self, index: usize) {
        if index < self.pending.len() {
            self.pending.remove(index);
        }
    }

    /// Restored tabs that no live tab stands for.
    fn not_live<'a>(&'a self, tm: &'a TabManager) -> impl Iterator<Item = &'a SessionTab> {
        self.pending
            .iter()
            .filter(|p| p.live.is_none_or(|id| tm.tab(id).is_none()))
            .map(|p| &p.tab)
    }

    /// The user closed this tab: stop restoring and re-saving it.
    pub fn forget(&mut self, id: TabId) {
        self.pending.retain(|p| p.live != Some(id));
    }

    /// Whether a restored tab for `conn` is still pending (so its tabs must not be reset).
    pub fn awaits(&self, conn: &ConnectionId) -> bool {
        self.pending
            .iter()
            .any(|p| p.tab.spec.conn_id() == Some(conn))
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::connection::ConnectionId;

    fn viewer(object: &str) -> TabSpec {
        TabSpec::DataViewer {
            conn_id: ConnectionId::parse("c"),
            object: based_core::ObjectRef::bare(object),
            filter: None,
        }
    }

    #[test]
    fn restore_runs_once() {
        let mut st = SessionState::default();
        assert!(st.begin_restore());
        assert!(!st.begin_restore());
    }

    #[test]
    fn pending_tabs_open_once_and_close_by_id() {
        let mut st = SessionState::default();
        st.set_pending([SessionTab::new(TabSpec::Home), SessionTab::new(viewer("t"))]);
        let to_open = st.to_open();
        assert_eq!(to_open.len(), 1, "Home is never restored");
        assert_eq!(to_open[0].1.spec, viewer("t"));
        let id = TabId::new();
        st.link(to_open[0].0, id);
        assert!(st.to_open().is_empty());
        assert!(st.awaits(&ConnectionId::parse("c")));
        st.forget(TabId::new());
        assert!(st.awaits(&ConnectionId::parse("c")), "another tab's id");
        st.forget(id);
        assert!(!st.awaits(&ConnectionId::parse("c")));
    }

    #[test]
    fn old_entries_read_as_unpinned_tabs_and_legacy_pins_apply_once() {
        let raw = r#"[
            {"type":"object_info","conn_id":"t","object_name":"a","kind":"trigger"},
            {"type":"object_info","conn_id":"t","object_name":"a","kind":"trigger"}
        ]"#;
        let mut tabs: Vec<SessionTab> = serde_json::from_str(raw).unwrap();
        assert!(tabs.iter().all(|t| !t.pinned));
        let spec = tabs[0].spec.clone();
        apply_legacy_pins(&mut tabs, &[spec]);
        assert_eq!(
            tabs.iter().map(|t| t.pinned).collect::<Vec<_>>(),
            [true, false],
            "one pinned spec pins one tab, not every equal one"
        );
        let json = serde_json::to_string(&tabs[0]).unwrap();
        assert!(json.contains(r#""pinned":true"#) && json.contains(r#""type":"object_info""#));
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
