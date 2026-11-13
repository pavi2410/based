//! Query execution that cannot write, for AI tools.

use anyhow::Result;
use futures::TryStreamExt;
use sqlx::{AssertSqlSafe, PgPool, Row};

use based_core::ResultSet;

use crate::decode::pg_cell;
use crate::mutations::result_columns;

const TIMEOUT_MS: u32 = 15_000;

/// Runs `sql` in a read-only transaction with a statement timeout and rolls it back.
/// Stops after `limit + 1` rows so callers can tell the result was cut off.
pub async fn query_readonly(pool: &PgPool, sql: &str, limit: usize) -> Result<ResultSet> {
    let mut tx = pool.begin().await?;
    sqlx::query("SET TRANSACTION READ ONLY")
        .execute(&mut *tx)
        .await?;
    sqlx::query(AssertSqlSafe(format!(
        "SET LOCAL statement_timeout = {TIMEOUT_MS}"
    )))
    .execute(&mut *tx)
    .await?;
    let mut set = ResultSet::default();
    {
        let mut stream = sqlx::query(AssertSqlSafe(sql)).fetch(&mut *tx);
        while let Some(row) = stream.try_next().await? {
            if set.columns.is_empty() {
                set.columns = result_columns(row.columns());
            }
            set.rows
                .push((0..row.len()).map(|i| pg_cell(&row, i)).collect());
            if set.rows.len() > limit {
                break;
            }
        }
    }
    tx.rollback().await?;
    Ok(set)
}
