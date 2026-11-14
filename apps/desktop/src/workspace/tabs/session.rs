//! Persisted session snapshot stored in the metadata SQLite database.

use serde::de::DeserializeOwned;
use serde::{Deserialize, Serialize};

use std::mem;
use std::path::Path;

use based_storage::{ACTIVE_CONNECTION_ID, ACTIVE_TAB_INDEX, MetadataStore, OPEN_TABS};

use super::manager::{TabId, TabManager};
use super::spec::TabSpec;
use crate::connection::ConnectionId;
use crate::project::scope::canonical_root;

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

/// Where a workspace's session is saved: one slot per project root, and one
/// for windows with no project.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SessionScope(String);

impl SessionScope {
    pub fn for_project(root: Option<&Path>) -> Self {
        Self(root.map_or_else(
            || "no-project".to_owned(),
            |root| format!("project:{}", canonical_root(root).display()),
        ))
    }

    fn key(&self, base: &str) -> String {
        format!("{}|{base}", self.0)
    }
}

/// Keys from before sessions were saved per project. Read once, by the first
/// scope that has no session of its own, then deleted.
const LEGACY_KEYS: [&str; 4] = [
    OPEN_TABS,
    ACTIVE_TAB_INDEX,
    ACTIVE_CONNECTION_ID,
    LEGACY_PINNED_TABS,
];

/// Load tabs saved under `key`, migrating old entry shapes and skipping any
/// that still fail to deserialize (e.g. pre-`QueryEditorInit` flat fields).
/// `None` when nothing was ever saved there.
async fn load_tabs_with_migration(store: &MetadataStore, key: &str) -> Option<Vec<SessionTab>> {
    let raw = store
        .get_session_json::<Vec<serde_json::Value>>(key)
        .await
        .ok()??;
    Some(
        raw.into_iter()
            .map(migrate_entry)
            .filter_map(|v| serde_json::from_value(v).ok())
            .collect(),
    )
}

/// Mongo pipelines used to be saved as a query editor with
/// `init.kind = "mongo_pipeline"`; they are `type = "pipeline"` now.
fn migrate_entry(mut v: serde_json::Value) -> serde_json::Value {
    let is_legacy_pipeline = v["type"] == "query_editor" && v["init"]["kind"] == "mongo_pipeline";
    if !is_legacy_pipeline {
        return v;
    }
    let init = v["init"].take();
    let mut out = serde_json::json!({ "type": "pipeline", "conn_id": v["conn_id"].take() });
    for key in ["collection", "pipeline"] {
        if !init[key].is_null() {
            out[key] = init[key].clone();
        }
    }
    if v["pinned"] == true {
        out["pinned"] = true.into();
    }
    out
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
    /// The session saved for `scope`. A scope that never saved one inherits the
    /// pre-scope session once (the keys are deleted after), so an upgrade keeps
    /// the user's tabs.
    pub async fn load(store: &MetadataStore, scope: &SessionScope) -> Self {
        if let Some(tabs) = load_tabs_with_migration(store, &scope.key(OPEN_TABS)).await {
            return Self {
                tabs,
                active: get(store, &scope.key(ACTIVE_TAB_INDEX)).await,
                active_connection_id: get(store, &scope.key(ACTIVE_CONNECTION_ID)).await,
            };
        }
        let Some(mut tabs) = load_tabs_with_migration(store, OPEN_TABS).await else {
            return Self::default();
        };
        let legacy_pins: Vec<TabSpec> = get(store, LEGACY_PINNED_TABS).await.unwrap_or_default();
        apply_legacy_pins(&mut tabs, &legacy_pins);
        let legacy = Self {
            tabs,
            active: get(store, ACTIVE_TAB_INDEX).await,
            active_connection_id: get(store, ACTIVE_CONNECTION_ID).await,
        };
        if let Err(err) = store.remove_session_keys(&LEGACY_KEYS).await {
            log::warn!("clearing the pre-scope session failed: {err:#}");
        }
        legacy
    }

    pub async fn save(&self, store: &MetadataStore, scope: &SessionScope) -> anyhow::Result<()> {
        store
            .set_session_json(&scope.key(OPEN_TABS), &self.tabs)
            .await?;
        store
            .set_session_json(&scope.key(ACTIVE_TAB_INDEX), &self.active)
            .await?;
        store
            .set_session_json(&scope.key(ACTIVE_CONNECTION_ID), &self.active_connection_id)
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

async fn get<T: DeserializeOwned>(store: &MetadataStore, key: &str) -> Option<T> {
    store.get_session_json(key).await.ok().flatten()
}

/// Restore bookkeeping and the coalescing write queue for one workspace.
#[derive(Default)]
pub struct SessionState {
    /// Where saves go. `None` while the window changes project: the tabs being
    /// closed for the switch must not overwrite the old project's session.
    scope: Option<SessionScope>,
    restored: bool,
    /// Saved tabs, kept until the user closes them: opening a connection can
    /// replace the center, and these bring the tabs back next launch.
    pending: Vec<PendingTab>,
    /// Newest snapshot waiting for the writer (newer replaces older).
    queued: Option<(SessionScope, SessionSnapshot)>,
    writing: bool,
}

impl SessionState {
    pub fn new(scope: SessionScope) -> Self {
        Self {
            scope: Some(scope),
            ..Self::default()
        }
    }

    /// Where to save now: `None` while held, and until the scope's saved
    /// session has been read, so a save never overwrites tabs not yet restored.
    pub fn save_scope(&self) -> Option<&SessionScope> {
        self.scope.as_ref().filter(|_| self.restored)
    }

    /// Stop saving until [`Self::resume`]: the window is changing project.
    pub fn hold(&mut self) {
        self.scope = None;
    }

    /// Save to `scope` from now on.
    pub fn resume(&mut self, scope: SessionScope) {
        self.scope = Some(scope);
    }

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

    /// Whether a restored tab is already linked to live tab `id`.
    pub fn is_linked(&self, id: TabId) -> bool {
        self.pending.iter().any(|p| p.live == Some(id))
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
    pub fn enqueue(&mut self, scope: SessionScope, snapshot: SessionSnapshot) -> bool {
        self.queued = Some((scope, snapshot));
        !mem::replace(&mut self.writing, true)
    }

    pub fn take_queued(&mut self) -> Option<(SessionScope, SessionSnapshot)> {
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
    fn legacy_mongo_pipeline_entries_become_pipeline_tabs() {
        let legacy = serde_json::json!({
            "type": "query_editor",
            "conn_id": "m",
            "init": {"kind": "mongo_pipeline", "pipeline": "[]", "collection": "orders"}
        });
        let tab: SessionTab = serde_json::from_value(migrate_entry(legacy)).unwrap();
        assert_eq!(
            tab.spec,
            TabSpec::Pipeline {
                conn_id: ConnectionId::parse("m"),
                collection: Some("orders".into()),
                pipeline: Some("[]".into()),
            }
        );
        let sql = serde_json::json!({
            "type": "query_editor",
            "conn_id": "p",
            "init": {"kind": "sql", "sql": "select 1"}
        });
        let tab: SessionTab = serde_json::from_value(migrate_entry(sql)).unwrap();
        let TabSpec::QueryEditor { init, .. } = tab.spec else {
            panic!("query editor");
        };
        assert_eq!(init.sql.as_deref(), Some("select 1"));
        let json = serde_json::to_value(&init).unwrap();
        assert_eq!(json["kind"], "sql", "older builds still read it");
    }

    #[test]
    fn writer_starts_once_and_keeps_only_the_newest_snapshot() {
        let mut st = SessionState::default();
        let snap = |n: usize| SessionSnapshot {
            active: Some(n),
            ..Default::default()
        };
        let scope = SessionScope::for_project(None);
        assert!(
            st.enqueue(scope.clone(), snap(1)),
            "first enqueue starts the writer"
        );
        assert!(
            !st.enqueue(scope.clone(), snap(2)),
            "writer already running"
        );
        assert_eq!(st.take_queued().unwrap().1.active, Some(2));
        assert!(st.take_queued().is_none());
        st.finish_writing();
        assert!(st.enqueue(scope, snap(3)));
    }

    #[test]
    fn nothing_is_saved_before_the_scope_is_restored_or_while_held() {
        let scope = SessionScope::for_project(None);
        let mut st = SessionState::new(scope.clone());
        assert!(st.save_scope().is_none(), "saved tabs not read yet");
        st.begin_restore();
        assert_eq!(st.save_scope(), Some(&scope));
        st.hold();
        assert!(st.save_scope().is_none());
        st.resume(scope.clone());
        assert_eq!(st.save_scope(), Some(&scope));
    }

    #[test]
    fn each_project_has_its_own_slot() {
        let a = SessionScope::for_project(Some(Path::new("/a")));
        let b = SessionScope::for_project(Some(Path::new("/b")));
        let none = SessionScope::for_project(None);
        assert_ne!(a.key(OPEN_TABS), b.key(OPEN_TABS));
        assert_ne!(a.key(OPEN_TABS), none.key(OPEN_TABS));
        assert_ne!(none.key(OPEN_TABS), OPEN_TABS, "never the pre-scope key");
    }
}
