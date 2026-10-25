//! Query execution that cannot write, for AI tools.

use anyhow::Result;
use futures::TryStreamExt;
use sqlx::{AssertSqlSafe, Column, Row, SqliteConnection, SqlitePool};

use crate::page::sqlite_cell_display;

/// Runs `sql` on a connection with `PRAGMA query_only` set (restored afterwards).
/// Stops after `limit + 1` rows so callers can tell the result was cut off.
pub async fn query_readonly(
    pool: &SqlitePool,
    sql: &str,
    limit: usize,
) -> Result<(Vec<String>, Vec<Vec<String>>)> {
    let mut conn = pool.acquire().await?;
    sqlx::query("PRAGMA query_only = ON")
        .execute(&mut *conn)
        .await?;
    let result = collect(&mut conn, sql, limit).await;
    // Pooled connections are reused, so the guard must always come off.
    sqlx::query("PRAGMA query_only = OFF")
        .execute(&mut *conn)
        .await?;
    result
}

async fn collect(
    conn: &mut SqliteConnection,
    sql: &str,
    limit: usize,
) -> Result<(Vec<String>, Vec<Vec<String>>)> {
    let mut columns = Vec::new();
    let mut rows = Vec::new();
    let mut stream = sqlx::query(AssertSqlSafe(sql)).fetch(conn);
    while let Some(row) = stream.try_next().await? {
        if columns.is_empty() {
            columns = row.columns().iter().map(|c| c.name().to_owned()).collect();
        }
        rows.push(
            (0..row.len())
                .map(|i| sqlite_cell_display(&row, i))
                .collect(),
        );
        if rows.len() > limit {
            break;
        }
    }
    Ok((columns, rows))
}

#[cfg(test)]
mod tests {
    use super::*;

    async fn pool() -> SqlitePool {
        let pool = SqlitePool::connect("sqlite::memory:").await.unwrap();
        sqlx::query("CREATE TABLE t (a INT)")
            .execute(&pool)
            .await
            .unwrap();
        sqlx::query("INSERT INTO t VALUES (1),(2),(3)")
            .execute(&pool)
            .await
            .unwrap();
        pool
    }

    #[tokio::test]
    async fn reads_and_caps_rows() {
        let pool = pool().await;
        let (cols, rows) = query_readonly(&pool, "SELECT a FROM t", 2).await.unwrap();
        assert_eq!(cols, vec!["a"]);
        assert_eq!(rows.len(), 3); // limit + 1 marks truncation
    }

    #[tokio::test]
    async fn refuses_writes_and_restores_the_connection() {
        let pool = pool().await;
        assert!(query_readonly(&pool, "DELETE FROM t", 10).await.is_err());
        let (_, rows) = query_readonly(&pool, "SELECT a FROM t", 10).await.unwrap();
        assert_eq!(rows.len(), 3);
        // The pool is usable for writes again.
        sqlx::query("INSERT INTO t VALUES (4)")
            .execute(&pool)
            .await
            .unwrap();
    }
}
