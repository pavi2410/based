use std::collections::HashSet;
use std::fs;
use std::path::{Path, PathBuf};

use based_project::{ProjectQuery, ProjectSnapshot, persist_favorites};

pub use based_query::{HistoryEntry, QueryHistory};

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

#[cfg(test)]
mod tests {
    use super::*;
    use std::env;
    use std::sync::Mutex;
    use std::time::Duration;

    static CWD_LOCK: Mutex<()> = Mutex::new(());

    fn sample_history(sql: &str) -> HistoryEntry {
        HistoryEntry::new(
            based_core::ConnectionId::parse("ws-template:x"),
            sql,
            Duration::from_millis(1),
            based_query::RunOutcome::Ok { rows: Some(0) },
        )
    }

    #[test]
    fn no_project_history_stays_in_memory() {
        let _cwd_lock = CWD_LOCK.lock().unwrap();
        let cwd = tempfile::tempdir().unwrap();
        let prev = env::current_dir().unwrap();
        env::set_current_dir(cwd.path()).unwrap();
        let mut store = QueryStore::new(None, None);
        store.push_history(sample_history("SELECT 1"));
        let leaked =
            cwd.path().join(".based").exists() || cwd.path().join("history.jsonl").exists();
        env::set_current_dir(prev).unwrap();
        assert!(!leaked, "no-project history must not write cwd files");
        assert_eq!(store.history.recent(1)[0].query, "SELECT 1");
    }
}
