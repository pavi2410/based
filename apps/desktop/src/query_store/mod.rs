//! Saved queries (`queries.toml`) and per-run history (`local/history.jsonl`).

pub mod history;
pub mod saved;

pub use history::{HistoryEntry, QueryHistory};
pub use saved::{SavedQueries, SavedQuery};

use std::path::PathBuf;

use gpui::{App, Global};

use crate::connection::ConnectionId;

/// Signals for observers once UI subscribes (CommandPalette, inspector, etc.).
#[allow(dead_code)]
#[derive(Clone, Debug)]
pub enum QueryStoreEvent {
    HistoryUpdated(ConnectionId),
    SavedUpdated,
}

pub struct QueryStore {
    pub history: QueryHistory,
    pub saved: SavedQueries,
    queries_dir: PathBuf,
    saved_path: PathBuf,
}

impl QueryStore {
    pub fn new(project_root: Option<PathBuf>) -> Self {
        let base = project_root.unwrap_or_else(|| PathBuf::from("."));
        let queries_dir = base.join(".based").join("local");
        let saved_path = base.join(".based").join("queries.toml");

        let _ = std::fs::create_dir_all(&queries_dir);

        let gitignore = base.join(".based").join(".gitignore");
        if !gitignore.exists() {
            let _ = std::fs::write(&gitignore, "local/\n");
        }

        Self {
            history: QueryHistory::load(&queries_dir),
            saved: SavedQueries::load(&saved_path),
            queries_dir,
            saved_path,
        }
    }

    pub fn push_history(&mut self, entry: HistoryEntry) {
        self.history.push(entry, &self.queries_dir);
    }

    pub fn save_query(&mut self, query: SavedQuery) {
        self.saved.add(query);
        self.saved.persist(&self.saved_path);
    }

    pub fn history_for(&self, conn_id: &ConnectionId) -> Vec<&HistoryEntry> {
        self.history.for_conn(conn_id)
    }

    pub fn all_saved(&self) -> &[SavedQuery] {
        &self.saved.queries
    }
}

impl Global for QueryStore {}

pub fn init(project_root: Option<std::path::PathBuf>, cx: &mut App) {
    cx.set_global(QueryStore::new(project_root));
}
