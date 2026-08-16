//! SQLite PRAGMA snapshot used by the connection dashboard.

use sqlx::{AssertSqlSafe, Row, SqlitePool};

use crate::widgets::row_cell::sqlite_cell_display;

const PRAGMA_LIST: &[&str] = &[
    "page_size",
    "page_count",
    "journal_mode",
    "synchronous",
    "cache_size",
    "auto_vacuum",
    "freelist_count",
    "integrity_check",
    "wal_checkpoint",
];

/// Read the dashboard PRAGMA set from `pool`.
pub async fn fetch_sqlite_pragmas(pool: &SqlitePool) -> Vec<(String, String)> {
    let mut rows = Vec::with_capacity(PRAGMA_LIST.len());
    for &name in PRAGMA_LIST {
        let sql = format!("PRAGMA {name}");
        let value = match sqlx::query(AssertSqlSafe(sql)).fetch_optional(pool).await {
            Ok(Some(row)) => {
                let parts: Vec<String> = (0..row.len())
                    .map(|i| sqlite_cell_display(&row, i))
                    .collect();
                parts.join(", ")
            }
            Ok(None) | Err(_) => String::new(),
        };
        rows.push((name.to_string(), value));
    }
    rows
}

#[cfg(test)]
mod tests {
    use super::*;

    async fn mem_pool() -> SqlitePool {
        SqlitePool::connect("sqlite::memory:").await.unwrap()
    }

    #[tokio::test]
    async fn snapshot_includes_every_dashboard_pragma() {
        let pool = mem_pool().await;
        let rows = fetch_sqlite_pragmas(&pool).await;
        let names: Vec<&str> = rows.iter().map(|(k, _)| k.as_str()).collect();
        assert_eq!(names, PRAGMA_LIST);
        assert!(rows.iter().all(|(_, v)| !v.is_empty()));
    }

    #[tokio::test]
    async fn memory_db_reports_memory_journal_and_ok_integrity() {
        let pool = mem_pool().await;
        let rows = fetch_sqlite_pragmas(&pool).await;
        let value = |name: &str| {
            rows.iter()
                .find(|(k, _)| k == name)
                .map(|(_, v)| v.as_str())
                .unwrap()
        };
        assert_eq!(value("journal_mode"), "memory");
        assert_eq!(value("integrity_check"), "ok");
    }
}
