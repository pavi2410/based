use std::collections::HashSet;
use std::fs;
use std::path::{Path, PathBuf};

pub mod history;

use based_project::{ProjectQuery, ProjectSnapshot, persist_favorites};
use gpui::{App, Global};

pub use history::{HistoryEntry, QueryHistory};

/// In-memory catalog of committed project queries + user favorites + run history.
pub struct QueryStore {
    pub history: QueryHistory,
    pub queries: Vec<ProjectQuery>,
    pub favorites: HashSet<String>,
    /// `.based/local` when a project is open; `None` keeps history in memory only.
    pub(crate) history_dir: Option<PathBuf>,
}

impl QueryStore {
    pub fn new(project_root: Option<PathBuf>, snapshot: Option<&ProjectSnapshot>) -> Self {
        let history_dir = project_root.map(|base| {
            let dir = base.join(".based").join("local");
            let _ = fs::create_dir_all(&dir);
            dir
        });

        let (queries, favorites) = snapshot
            .map(|s| (s.queries.clone(), s.favorites.iter().cloned().collect()))
            .unwrap_or_default();

        Self {
            history: history_dir
                .as_deref()
                .map(QueryHistory::load)
                .unwrap_or_else(QueryHistory::empty),
            queries,
            favorites,
            history_dir,
        }
    }

    pub fn apply_snapshot(&mut self, snapshot: &ProjectSnapshot) {
        self.queries = snapshot.queries.clone();
        self.favorites = snapshot.favorites.iter().cloned().collect();
    }

    /// Drop project-owned queries, favorites, and history after Close Project.
    pub fn clear_project(&mut self) {
        self.queries.clear();
        self.favorites.clear();
        self.history = QueryHistory::empty();
        self.history_dir = None;
    }

    pub fn project_queries(&self) -> &[ProjectQuery] {
        &self.queries
    }

    pub fn is_favorite(&self, path: &str) -> bool {
        self.favorites.contains(path)
    }

    pub fn toggle_favorite(&mut self, project_root: &Path, path: &str) -> bool {
        if self.favorites.contains(path) {
            self.favorites.remove(path);
        } else {
            self.favorites.insert(path.to_string());
        }
        let ordered: Vec<String> = self.favorites.iter().cloned().collect();
        let _ = persist_favorites(project_root, &ordered);
        self.favorites.contains(path)
    }

    pub fn push_history(&mut self, entry: HistoryEntry) {
        self.history.push(entry, self.history_dir.as_deref());
    }

    pub fn history_for(&self, conn_id: &based_core::ConnectionId) -> Vec<&HistoryEntry> {
        self.history.for_conn(conn_id)
    }
}

impl Global for QueryStore {}

pub fn init(project_root: Option<PathBuf>, snapshot: Option<ProjectSnapshot>, cx: &mut App) {
    let snap_ref = snapshot.as_ref();
    cx.set_global(QueryStore::new(project_root, snap_ref));
}

#[cfg(test)]
mod tests {
    use super::*;
    use based_project::{ProjectQuery, QueryBody, QueryTarget};
    use std::sync::Mutex;

    static CWD_LOCK: Mutex<()> = Mutex::new(());

    fn sample_query() -> ProjectQuery {
        ProjectQuery {
            path: "local/recent".into(),
            name: "recent".into(),
            description: None,
            tags: vec![],
            target: QueryTarget {
                connection: None,
                engine: None,
                tags: vec![],
                exclude_tags: vec![],
            },
            body: QueryBody::Sql {
                query: "SELECT 1".into(),
            },
        }
    }

    #[test]
    fn clear_project_drops_queries_favorites_and_history() {
        let mut store = QueryStore {
            history: QueryHistory::empty(),
            queries: vec![sample_query()],
            favorites: ["local/recent".into()].into(),
            history_dir: Some(PathBuf::from("/tmp/based-closed-project")),
        };
        store.clear_project();
        assert!(store.queries.is_empty());
        assert!(store.favorites.is_empty());
        assert!(store.history.recent(10).is_empty());
        assert!(store.history_dir.is_none());
    }

    fn sample_history(sql: &str) -> HistoryEntry {
        HistoryEntry::new(
            based_core::ConnectionId("ws-template:x".into()),
            sql,
            1,
            Some(0),
            based_query::RunStatus::Ok,
        )
    }

    #[test]
    fn no_project_history_stays_in_memory() {
        let _cwd_lock = CWD_LOCK.lock().unwrap();
        let cwd = tempfile::tempdir().unwrap();
        let prev = std::env::current_dir().unwrap();
        std::env::set_current_dir(cwd.path()).unwrap();
        let mut store = QueryStore::new(None, None);
        store.push_history(sample_history("SELECT 1"));
        let leaked =
            cwd.path().join(".based").exists() || cwd.path().join("history.jsonl").exists();
        std::env::set_current_dir(prev).unwrap();
        assert!(!leaked, "no-project history must not write cwd files");
        assert_eq!(store.history.recent(1)[0].query, "SELECT 1");
    }

    #[test]
    fn closed_project_history_stays_in_memory() {
        let _cwd_lock = CWD_LOCK.lock().unwrap();
        let project = tempfile::tempdir().unwrap();
        let mut store = QueryStore::new(Some(project.path().to_path_buf()), None);
        store.push_history(sample_history("SELECT 1"));
        let hist = project
            .path()
            .join(".based")
            .join("local")
            .join("history.jsonl");
        let before = fs::read_to_string(&hist).unwrap();
        store.clear_project();

        let cwd = tempfile::tempdir().unwrap();
        let prev = std::env::current_dir().unwrap();
        std::env::set_current_dir(cwd.path()).unwrap();
        store.push_history(sample_history("SELECT 2"));
        let leaked =
            cwd.path().join(".based").exists() || cwd.path().join("history.jsonl").exists();
        std::env::set_current_dir(prev).unwrap();

        assert!(!leaked, "closed-project history must not write cwd files");
        assert_eq!(fs::read_to_string(&hist).unwrap(), before);
        assert_eq!(store.history.recent(1)[0].query, "SELECT 2");
    }
}
