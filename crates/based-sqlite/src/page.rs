//! Table browse page fetch and cell formatting (no UI).

use anyhow::Result;
use based_core::RawPage;
use sqlx::{AssertSqlSafe, Column, Row, SqlitePool, sqlite::SqliteRow};

const NULL_CELL: &str = "NULL";

fn show<T: ToString>(v: Option<T>) -> String {
    v.map_or_else(|| NULL_CELL.into(), |x| x.to_string())
}

/// Format a column from a SQLite row.
pub fn sqlite_cell_display(row: &SqliteRow, col: usize) -> String {
    if let Ok(v) = row.try_get::<Option<i64>, _>(col) {
        return show(v);
    }
    if let Ok(v) = row.try_get::<Option<f64>, _>(col) {
        return show(v);
    }
    if let Ok(v) = row.try_get::<Option<bool>, _>(col) {
        return show(v);
    }
    if let Ok(v) = row.try_get::<Option<String>, _>(col) {
        return v.unwrap_or_else(|| NULL_CELL.into());
    }
    if let Ok(v) = row.try_get::<Option<Vec<u8>>, _>(col) {
        return v.map_or_else(|| NULL_CELL.into(), |b| format!("<{} bytes>", b.len()));
    }
    NULL_CELL.into()
}

/// `table` must be a quoted identifier; `where_sql` is empty or ` WHERE …`.
pub async fn fetch_page(
    pool: &SqlitePool,
    table: &str,
    where_sql: &str,
    limit: u64,
    offset: u64,
) -> Result<RawPage> {
    let total: i64 = sqlx::query_scalar(AssertSqlSafe(format!(
        "SELECT COUNT(*) FROM {table}{where_sql}"
    )))
    .fetch_one(pool)
    .await
    .unwrap_or(0);
    let rows = sqlx::query(AssertSqlSafe(format!(
        "SELECT * FROM {table}{where_sql} LIMIT {limit} OFFSET {offset}"
    )))
    .fetch_all(pool)
    .await?;
    Ok(RawPage {
        total,
        columns: rows
            .first()
            .map(|r| r.columns().iter().map(|c| c.name().to_string()).collect()),
        rows: rows
            .iter()
            .map(|r| (0..r.len()).map(|i| sqlite_cell_display(r, i)).collect())
            .collect(),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn pages_and_filters() {
        let pool = SqlitePool::connect("sqlite::memory:").await.unwrap();
        sqlx::query("CREATE TABLE t (id INTEGER, name TEXT)")
            .execute(&pool)
            .await
            .unwrap();
        for i in 0..5 {
            sqlx::query("INSERT INTO t VALUES (?, NULL)")
                .bind(i)
                .execute(&pool)
                .await
                .unwrap();
        }
        let p = fetch_page(&pool, "\"t\"", "", 2, 4).await.unwrap();
        assert_eq!(p.total, 5);
        assert_eq!(p.columns, Some(vec!["id".into(), "name".into()]));
        assert_eq!(p.rows, vec![vec!["4".to_string(), "NULL".to_string()]]);
        let p = fetch_page(&pool, "\"t\"", " WHERE id > 9", 10, 0)
            .await
            .unwrap();
        assert_eq!((p.total, p.columns, p.rows.len()), (0, None, 0));
    }
}
