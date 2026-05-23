// Query execution history persisted as newline-delimited JSON under `.based/local/history.jsonl`.

use std::io::{BufRead, BufReader, Write};
use std::path::Path;

use serde::{Deserialize, Serialize};
use time::OffsetDateTime;

use crate::connection::ConnectionId;

const MAX_HISTORY: usize = 500;

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct HistoryEntry {
    pub conn_id: ConnectionId,
    /// SQL text or pipeline JSON string
    pub query: String,
    #[serde(with = "time::serde::rfc3339")]
    pub ran_at: OffsetDateTime,
    pub duration_ms: u64,
    pub row_count: Option<u64>,
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
            .collect::<Vec<_>>();
        Self { entries }
    }

    pub fn push(&mut self, entry: HistoryEntry, local_dir: &Path) {
        let conn_id = entry.conn_id.clone();
        self.entries.push(entry);
        let mut per_conn = 0usize;
        for e in self.entries.iter().rev() {
            if e.conn_id == conn_id {
                per_conn += 1;
                if per_conn > MAX_HISTORY {
                    let idx = self
                        .entries
                        .iter()
                        .position(|x| x.conn_id == conn_id)
                        .expect("conn entry exists");
                    self.entries.remove(idx);
                    break;
                }
            }
        }
        persist_history_slice(&self.entries, local_dir);
    }

    pub fn for_conn(&self, conn_id: &ConnectionId) -> Vec<&HistoryEntry> {
        self.entries
            .iter()
            .filter(|e| &e.conn_id == conn_id)
            .rev()
            .take(MAX_HISTORY)
            .collect()
    }

    pub fn recent(&self, limit: usize) -> Vec<&HistoryEntry> {
        self.entries.iter().rev().take(limit).collect()
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
            conn_id: ConnectionId(conn.into()),
            query: sql.into(),
            ran_at: OffsetDateTime::now_utc(),
            duration_ms: 10,
            row_count: Some(5),
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
        assert_eq!(results[0].query, "SELECT 2"); // most recent first
    }

    #[test]
    fn persists_and_reloads() {
        let dir = tempdir().unwrap();
        {
            let mut h = QueryHistory::load(dir.path());
            h.push(entry("pg", "SELECT 1"), dir.path());
        }
        let h2 = QueryHistory::load(dir.path());
        assert_eq!(h2.entries.len(), 1);
        assert_eq!(h2.entries[0].query, "SELECT 1");
    }

    #[test]
    fn caps_at_500_per_connection() {
        let dir = tempdir().unwrap();
        let mut h = QueryHistory::load(dir.path());
        for i in 0..510 {
            h.push(entry("pg", &format!("SELECT {i}")), dir.path());
        }
        assert_eq!(h.entries.len(), 500);
        h.push(entry("sqlite", "SELECT 1"), dir.path());
        assert_eq!(h.entries.len(), 501);
        for i in 0..510 {
            h.push(entry("sqlite", &format!("SELECT s{i}")), dir.path());
        }
        assert_eq!(h.for_conn(&ConnectionId("sqlite".into())).len(), 500);
        assert_eq!(h.for_conn(&ConnectionId("pg".into())).len(), 500);
    }
}
