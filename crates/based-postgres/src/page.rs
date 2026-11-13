//! Table browse page fetch (no UI).

use anyhow::Result;
use based_core::{RawPage, ResultSet};
use sqlx::postgres::PgRow;
use sqlx::{AssertSqlSafe, PgPool, Row};

use crate::decode::pg_cell;
use crate::mutations::result_columns;

/// `table` must be a quoted (schema-qualified) identifier; `where_sql` is empty or
/// ` WHERE …`; `order_sql` is empty or ` ORDER BY …` (see `based_core::page_order_sql`).
pub async fn fetch_page(
    pool: &PgPool,
    table: &str,
    where_sql: &str,
    order_sql: &str,
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
        "SELECT * FROM {table}{where_sql}{order_sql} LIMIT {limit} OFFSET {offset}"
    )))
    .fetch_all(pool)
    .await?;
    Ok(RawPage {
        total,
        set: result_set(&rows),
    })
}

/// Rows as a result set; no rows means no columns.
pub(crate) fn result_set(rows: &[PgRow]) -> ResultSet {
    ResultSet {
        columns: rows
            .first()
            .map(|r| result_columns(r.columns()))
            .unwrap_or_default(),
        rows: rows
            .iter()
            .map(|r| (0..r.len()).map(|i| pg_cell(r, i)).collect())
            .collect(),
    }
}
