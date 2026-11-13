use anyhow::Result;
use based_core::{ResultColumn, ResultSet, SqlOutcome};
use futures::TryStreamExt;
use sqlx::sqlite::SqliteColumn;
use sqlx::{
    AssertSqlSafe, Column as SqlxColumn, Either, Executor, Row, SqlSafeStr, SqlitePool, TypeInfo,
};

use crate::page::sqlite_cell;

pub async fn insert_row(
    pool: &SqlitePool,
    table: &str,
    values: &[(String, String)],
) -> Result<i64> {
    let cols: Vec<&str> = values.iter().map(|(k, _)| k.as_str()).collect();
    let placeholders: Vec<String> = (1..=values.len()).map(|i| format!("?{i}")).collect();

    let sql = format!(
        "INSERT INTO \"{table}\" ({}) VALUES ({})",
        cols.join(", "),
        placeholders.join(", ")
    );

    let mut q = sqlx::query(AssertSqlSafe(sql));
    for (_, v) in values {
        q = q.bind(v);
    }
    let result = q.execute(pool).await?;
    Ok(result.last_insert_rowid())
}

pub async fn update_row(
    pool: &SqlitePool,
    table: &str,
    pk_col: &str,
    pk_val: &str,
    changes: &[(String, String)],
) -> Result<u64> {
    let set_clauses: Vec<String> = changes
        .iter()
        .enumerate()
        .map(|(i, (col, _))| format!("\"{col}\" = ?{}", i + 1))
        .collect();

    let pk_bind_idx = changes.len() + 1;
    let sql = format!(
        "UPDATE \"{table}\" SET {} WHERE \"{pk_col}\" = ?{pk_bind_idx}",
        set_clauses.join(", ")
    );

    let mut q = sqlx::query(AssertSqlSafe(sql));
    for (_, v) in changes {
        q = q.bind(v);
    }
    q = q.bind(pk_val);
    let result = q.execute(pool).await?;
    Ok(result.rows_affected())
}

pub async fn delete_row(pool: &SqlitePool, table: &str, pk_col: &str, pk_val: &str) -> Result<u64> {
    let sql = format!("DELETE FROM \"{table}\" WHERE \"{pk_col}\" = ?1");
    let result = sqlx::query(AssertSqlSafe(sql))
        .bind(pk_val)
        .execute(pool)
        .await?;
    Ok(result.rows_affected())
}

/// Runs one statement and returns whatever it produced.
///
/// SQLite decides whether a statement returns rows (`SELECT`, `VALUES`,
/// `PRAGMA`, `… RETURNING`), not the SQL text. When no rows come back,
/// `describe` tells a row statement with an empty result (its columns) from a
/// write (no columns).
pub async fn execute_sql(pool: &SqlitePool, sql: &str) -> Result<SqlOutcome> {
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
                        .push((0..row.len()).map(|i| sqlite_cell(&row, i)).collect());
                }
            }
        }
    }
    if set.rows.is_empty()
        && let Ok(described) = conn.describe(AssertSqlSafe(sql).into_sql_str()).await
    {
        set.columns = result_columns(described.columns());
    }
    Ok(if set.columns.is_empty() {
        SqlOutcome::Affected(affected)
    } else {
        SqlOutcome::Rows(set)
    })
}

pub(crate) fn result_columns(cols: &[SqliteColumn]) -> Vec<ResultColumn> {
    cols.iter()
        .map(|c| ResultColumn::new(c.name(), Some(c.type_info().name().to_string())))
        .collect()
}

#[cfg(test)]
mod tests {
    use super::execute_sql;
    use based_core::{SqlOutcome, Value};
    use sqlx::sqlite::SqlitePoolOptions;

    #[tokio::test]
    async fn the_server_decides_rows_or_affected() {
        // One connection: each in-memory connection is its own database.
        let pool = SqlitePoolOptions::new()
            .max_connections(1)
            .connect("sqlite::memory:")
            .await
            .unwrap();
        let run = |sql: &'static str| {
            let pool = pool.clone();
            async move { execute_sql(&pool, sql).await.unwrap() }
        };
        assert_eq!(
            run("CREATE TABLE t (id INTEGER, name TEXT)").await,
            SqlOutcome::Affected(0)
        );
        assert_eq!(
            run("INSERT INTO t VALUES (1, 'a'), (2, 'b')").await,
            SqlOutcome::Affected(2)
        );
        let SqlOutcome::Rows(set) = run("SELECT id FROM t WHERE id > 5").await else {
            panic!("an empty SELECT still returns rows");
        };
        assert_eq!(set.column_names().collect::<Vec<_>>(), ["id"]);
        assert!(set.rows.is_empty());
        let SqlOutcome::Rows(set) = run("PRAGMA table_info(t)").await else {
            panic!("PRAGMA returns rows");
        };
        assert_eq!(set.rows.len(), 2);
        let SqlOutcome::Rows(set) = run("UPDATE t SET name = 'z' WHERE id = 1 RETURNING id").await
        else {
            panic!("RETURNING returns rows");
        };
        assert_eq!(set.rows, vec![vec![Value::Int(1)]]);
    }
}
