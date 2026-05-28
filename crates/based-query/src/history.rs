use std::io::{BufRead, BufReader, Write};
use std::path::Path;

use based_core::ConnectionId;
use serde::{Deserialize, Serialize};
use time::OffsetDateTime;
use uuid::Uuid;

pub const MAX_HISTORY_PER_CONNECTION: usize = 500;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RunStatus {
    #[default]
    Ok,
    Error,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct HistoryEntry {
    #[serde(default = "new_entry_id")]
    pub id: Uuid,
    pub conn_id: ConnectionId,
    #[serde(default)]
    pub database: Option<String>,
    pub query: String,
    #[serde(with = "time::serde::rfc3339")]
    pub ran_at: OffsetDateTime,
    pub duration_ms: u64,
    pub row_count: Option<u64>,
    #[serde(default)]
    pub status: RunStatus,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub error_summary: Option<String>,
    #[serde(default)]
    pub pinned: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub label: Option<String>,
}

fn new_entry_id() -> Uuid {
    Uuid::new_v4()
}

impl HistoryEntry {
    pub fn new(
        conn_id: ConnectionId,
        query: impl Into<String>,
        duration_ms: u64,
        row_count: Option<u64>,
        status: RunStatus,
    ) -> Self {
        Self {
            id: Uuid::new_v4(),
            conn_id,
            database: None,
            query: query.into(),
            ran_at: OffsetDateTime::now_utc(),
            duration_ms,
            row_count,
            status,
            error_summary: None,
            pinned: false,
            label: None,
        }
    }
}

pub struct QueryHistory {
    entries: Vec<HistoryEntry>,
}

impl QueryHistory {
    pub fn load(local_dir: &Path) -> Self {
        let path = history_path(local_dir);
        if !path.exists() {
            return Self { entries: vec![] };
        }
        let file = match std::fs::File::open(&path) {
            Ok(f) => f,
            Err(_) => return Self { entries: vec![] },
        };
        let entries = BufReader::new(file)
            .lines()
            .map_while(Result::ok)
            .filter_map(|l| serde_json::from_str(&l).ok())
            .collect();
        Self { entries }
    }

    pub fn push(&mut self, entry: HistoryEntry, local_dir: &Path) {
        let conn_id = entry.conn_id.clone();
        self.entries.push(entry);
        trim_connection(&mut self.entries, &conn_id);
        persist_history_slice(&self.entries, local_dir);
    }

    pub fn for_conn(&self, conn_id: &ConnectionId) -> Vec<&HistoryEntry> {
        self.entries
            .iter()
            .filter(|e| &e.conn_id == conn_id)
            .rev()
            .take(MAX_HISTORY_PER_CONNECTION)
            .collect()
    }

    pub fn recent(&self, limit: usize) -> Vec<&HistoryEntry> {
        self.entries.iter().rev().take(limit).collect()
    }

    pub fn search(&self, needle: &str) -> Vec<&HistoryEntry> {
        let needle = needle.to_ascii_lowercase();
        if needle.is_empty() {
            return self.recent(MAX_HISTORY_PER_CONNECTION);
        }
        self.entries
            .iter()
            .rev()
            .filter(|e| {
                e.query.to_ascii_lowercase().contains(&needle)
                    || e.label
                        .as_ref()
                        .is_some_and(|l| l.to_ascii_lowercase().contains(&needle))
            })
            .take(MAX_HISTORY_PER_CONNECTION)
            .collect()
    }

    pub fn set_pinned(&mut self, id: Uuid, pinned: bool, local_dir: &Path) -> bool {
        let Some(entry) = self.entries.iter_mut().find(|e| e.id == id) else {
            return false;
        };
        entry.pinned = pinned;
        persist_history_slice(&self.entries, local_dir);
        true
    }

    pub fn pinned_for_conn(&self, conn_id: &ConnectionId) -> Vec<&HistoryEntry> {
        self.entries
            .iter()
            .filter(|e| &e.conn_id == conn_id && e.pinned)
            .rev()
            .collect()
    }
}

fn trim_connection(entries: &mut Vec<HistoryEntry>, conn_id: &ConnectionId) {
    let mut per_conn = 0usize;
    for e in entries.iter().rev() {
        if &e.conn_id == conn_id {
            per_conn += 1;
            if per_conn > MAX_HISTORY_PER_CONNECTION {
                if let Some(idx) = entries.iter().position(|x| &x.conn_id == conn_id) {
                    entries.remove(idx);
                }
                break;
            }
        }
    }
}

fn history_path(local_dir: &Path) -> std::path::PathBuf {
    local_dir.join("history.jsonl")
}

fn persist_history_slice(entries: &[HistoryEntry], local_dir: &Path) {
    let _ = std::fs::create_dir_all(local_dir);
    let path = history_path(local_dir);
    let Ok(mut file) = std::fs::File::create(&path) else {
        return;
    };
    for e in entries {
        if let Ok(line) = serde_json::to_string(e) {
            let _ = writeln!(file, "{line}");
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    fn entry(conn: &str, sql: &str) -> HistoryEntry {
        HistoryEntry {
            id: Uuid::new_v4(),
            conn_id: ConnectionId(conn.into()),
            database: None,
            query: sql.into(),
            ran_at: OffsetDateTime::now_utc(),
            duration_ms: 10,
            row_count: Some(5),
            status: RunStatus::Ok,
            error_summary: None,
            pinned: false,
            label: None,
        }
    }

    #[test]
    fn push_and_retrieve() {
        let dir = tempdir().unwrap();
        let mut h = QueryHistory::load(dir.path());
        h.push(entry("pg", "SELECT 1"), dir.path());
        h.push(entry("pg", "SELECT 2"), dir.path());
        let results = h.for_conn(&ConnectionId("pg".into()));
        assert_eq!(results.len(), 2);
        assert_eq!(results[0].query, "SELECT 2");
    }

    #[test]
    fn pin_persists() {
        let dir = tempdir().unwrap();
        let mut h = QueryHistory::load(dir.path());
        let e = entry("pg", "SELECT 1");
        let id = e.id;
        h.push(e, dir.path());
        assert!(h.set_pinned(id, true, dir.path()));
        let h2 = QueryHistory::load(dir.path());
        assert!(h2.entries[0].pinned);
    }
}
