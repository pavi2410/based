pub mod history;

use std::collections::HashSet;
use std::path::{Path, PathBuf};

use based_project::{ProjectQuery, ProjectSnapshot, persist_favorites};
use gpui::Global;

pub use history::{HistoryEntry, QueryHistory};

/// In-memory catalog of committed project queries + user favorites + run history.
pub struct QueryStore {
    pub history: QueryHistory,
    pub queries: Vec<ProjectQuery>,
    pub favorites: HashSet<String>,
    pub(crate) history_dir: PathBuf,
}

impl QueryStore {
    pub fn new(project_root: Option<PathBuf>, snapshot: Option<&ProjectSnapshot>) -> Self {
        let base = project_root.clone().unwrap_or_else(|| PathBuf::from("."));
        let history_dir = base.join(".based").join("local");
        let _ = std::fs::create_dir_all(&history_dir);

        let (queries, favorites) = snapshot
            .map(|s| (s.queries.clone(), s.favorites.iter().cloned().collect()))
            .unwrap_or_default();

        Self {
            history: QueryHistory::load(&history_dir),
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
        self.history.push(entry, &self.history_dir);
    }

    pub fn history_for(&self, conn_id: &based_core::ConnectionId) -> Vec<&HistoryEntry> {
        self.history.for_conn(conn_id)
    }
}

impl Global for QueryStore {}

pub fn init(project_root: Option<PathBuf>, snapshot: Option<ProjectSnapshot>, cx: &mut gpui::App) {
    let snap_ref = snapshot.as_ref();
    cx.set_global(QueryStore::new(project_root, snap_ref));
}
