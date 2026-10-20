//! Table browse page fetch (no UI).

use anyhow::Result;
use based_core::RawPage;
use sqlx::{AssertSqlSafe, Column, PgPool, Row};

use crate::decode::pg_cell_display;

/// `table` must be a quoted (schema-qualified) identifier; `where_sql` is empty or ` WHERE …`.
pub async fn fetch_page(
    pool: &PgPool,
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
            .map(|r| (0..r.len()).map(|i| pg_cell_display(r, i)).collect())
            .collect(),
    })
}
