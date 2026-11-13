use anyhow::Result;
use futures::TryStreamExt;
use sqlx::postgres::PgColumn;
use sqlx::{
    AssertSqlSafe, Column as SqlxColumn, Either, Executor, PgPool, Row, SqlSafeStr, TypeInfo,
};

use based_core::{QueryError, ResultColumn, ResultSet, SqlOutcome};

use crate::decode::pg_cell;
use crate::error::query_error;

pub async fn insert_row(
    pool: &PgPool,
    schema: &str,
    table: &str,
    values: &[(String, String)],
) -> Result<u64> {
    if values.is_empty() {
        anyhow::bail!("no columns");
    }
    let cols: Vec<_> = values.iter().map(|(c, _)| c.as_str()).collect();
    let placeholders: Vec<_> = (1..=values.len()).map(|i| format!("${i}")).collect();
    let sql = format!(
        r#"INSERT INTO "{schema}"."{table}" ({}) VALUES ({})"#,
        cols.iter()
            .map(|c| format!(r#""{c}""#))
            .collect::<Vec<_>>()
            .join(", "),
        placeholders.join(", ")
    );
    let mut q = sqlx::query(AssertSqlSafe(sql));
    for (_, v) in values {
        q = q.bind(v);
    }
    let r = q.execute(pool).await?;
    Ok(r.rows_affected())
}

pub async fn delete_row(
    pool: &PgPool,
    schema: &str,
    table: &str,
    pk_col: &str,
    pk_val: &str,
) -> Result<u64> {
    let sql = format!(r#"DELETE FROM "{schema}"."{table}" WHERE "{pk_col}" = $1"#);
    let r = sqlx::query(AssertSqlSafe(sql))
        .bind(pk_val)
        .execute(pool)
        .await?;
    Ok(r.rows_affected())
}

/// Runs one statement and returns whatever it produced.
///
/// Postgres decides whether a statement returns rows (`SELECT`, `VALUES`, `TABLE`,
/// `… RETURNING`, set-returning functions, a leading comment …), so nothing here
/// guesses from the SQL text. When no rows come back, `describe` tells a row
/// statement with an empty result (its columns) from a write (no columns).
pub async fn execute_sql(pool: &PgPool, sql: &str) -> Result<SqlOutcome, QueryError> {
    run_statement(pool, sql)
        .await
        .map_err(|e| query_error(e, sql))
}

async fn run_statement(pool: &PgPool, sql: &str) -> Result<SqlOutcome, sqlx::Error> {
    // One connection for both calls: `describe` must see the same session
    // (temp tables, search_path) the statement just ran in.
    let mut conn = pool.acquire().await?;
    let mut set = ResultSet::default();
    let mut affected = 0u64;
    {
        let mut stream = conn.fetch_many(sqlx::query(AssertSqlSafe(sql)));
        while let Some(item) = stream.try_next().await? {
            match item {
                Either::Left(done) => affected += done.rows_affected(),
                Either::Right(row) => {
                    if set.columns.is_empty() {
                        set.columns = result_columns(row.columns());
                    }
                    set.rows
                        .push((0..row.len()).map(|i| pg_cell(&row, i)).collect());
                }
            }
        }
    }
    if set.rows.is_empty() {
        // The statement already succeeded, so a describe failure (e.g. it dropped
        // the table it read) just means it has no columns to show.
        if let Ok(described) = conn.describe(AssertSqlSafe(sql).into_sql_str()).await {
            set.columns = result_columns(described.columns());
        }
    }
    Ok(if set.columns.is_empty() {
        SqlOutcome::Affected(affected)
    } else {
        SqlOutcome::Rows(set)
    })
}

pub(crate) fn result_columns(cols: &[PgColumn]) -> Vec<ResultColumn> {
    cols.iter()
        .map(|c| ResultColumn::new(c.name(), Some(c.type_info().name().to_string())))
        .collect()
}
