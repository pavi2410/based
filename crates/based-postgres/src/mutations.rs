use anyhow::Result;
use futures::TryStreamExt;
use sqlx::postgres::PgColumn;
use sqlx::{
    AssertSqlSafe, Column as SqlxColumn, Either, Executor, PgPool, Row, SqlSafeStr, TypeInfo,
};

use crate::decode::pg_cell_display;

#[derive(Debug, Clone)]
pub struct QueryColumn {
    pub name: String,
    pub type_name: String,
}

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
/// guesses from the SQL text. When no rows come back, `describe` supplies the
/// column headers so an empty result set still shows its shape.
pub async fn execute_sql(
    pool: &PgPool,
    sql: &str,
) -> Result<(Vec<QueryColumn>, Vec<Vec<String>>, u64)> {
    // One connection for both calls: `describe` must see the same session
    // (temp tables, search_path) the statement just ran in.
    let mut conn = pool.acquire().await?;
    let mut columns: Vec<QueryColumn> = Vec::new();
    let mut data: Vec<Vec<String>> = Vec::new();
    let mut affected = 0u64;
    {
        let mut stream = conn.fetch_many(sqlx::query(AssertSqlSafe(sql)));
        while let Some(item) = stream.try_next().await? {
            match item {
                Either::Left(done) => affected += done.rows_affected(),
                Either::Right(row) => {
                    if columns.is_empty() {
                        columns = query_columns(row.columns());
                    }
                    data.push((0..row.len()).map(|i| pg_cell_display(&row, i)).collect());
                }
            }
        }
    }
    if data.is_empty() {
        // Headers for an empty result set. The statement already succeeded, so a
        // describe failure (e.g. it dropped the table it read) just means none.
        if let Ok(described) = conn.describe(AssertSqlSafe(sql).into_sql_str()).await {
            columns = query_columns(described.columns());
        }
    } else {
        // Postgres reports `SELECT n` as n rows affected; the grid already shows n.
        affected = 0;
    }
    Ok((columns, data, affected))
}

fn query_columns(cols: &[PgColumn]) -> Vec<QueryColumn> {
    cols.iter()
        .map(|c| QueryColumn {
            name: c.name().to_string(),
            type_name: c.type_info().name().to_string(),
        })
        .collect()
}
