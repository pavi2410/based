//! Query execution that cannot write, for AI tools.

use anyhow::Result;
use futures::TryStreamExt;
use sqlx::{AssertSqlSafe, Column, PgPool, Row};

use crate::decode::pg_cell_display;

const TIMEOUT_MS: u32 = 15_000;

/// Runs `sql` in a read-only transaction with a statement timeout and rolls it back.
/// Stops after `limit + 1` rows so callers can tell the result was cut off.
pub async fn query_readonly(
    pool: &PgPool,
    sql: &str,
    limit: usize,
) -> Result<(Vec<String>, Vec<Vec<String>>)> {
    let mut tx = pool.begin().await?;
    sqlx::query("SET TRANSACTION READ ONLY")
        .execute(&mut *tx)
        .await?;
    sqlx::query(AssertSqlSafe(format!(
        "SET LOCAL statement_timeout = {TIMEOUT_MS}"
    )))
    .execute(&mut *tx)
    .await?;
    let mut columns = Vec::new();
    let mut rows = Vec::new();
    {
        let mut stream = sqlx::query(AssertSqlSafe(sql)).fetch(&mut *tx);
        while let Some(row) = stream.try_next().await? {
            if columns.is_empty() {
                columns = row.columns().iter().map(|c| c.name().to_owned()).collect();
            }
            rows.push((0..row.len()).map(|i| pg_cell_display(&row, i)).collect());
            if rows.len() > limit {
                break;
            }
        }
    }
    tx.rollback().await?;
    Ok((columns, rows))
}
