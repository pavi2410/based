//! Table browse page fetch and cell formatting (no UI).

use anyhow::Result;
use based_core::{RawPage, Value};
use sqlx::{AssertSqlSafe, Column, Row, SqlitePool, TypeInfo, ValueRef, sqlite::SqliteRow};

/// Decode a column from a SQLite row by the storage class of *this* value.
///
/// SQLite is dynamically typed: a column declared `DATETIME` can hold an integer, so
/// the value's own class decides, not the declared type.
pub fn sqlite_cell(row: &SqliteRow, col: usize) -> Value {
    let Ok(raw) = row.try_get_raw(col) else {
        return Value::undecodable("unknown");
    };
    if raw.is_null() {
        return Value::Null;
    }
    let class = raw.type_info().name().to_string();
    let decoded = match class.as_str() {
        "INTEGER" | "BOOLEAN" => row.try_get_unchecked::<i64, _>(col).map(Value::Int),
        "REAL" => row.try_get_unchecked::<f64, _>(col).map(Value::Float),
        "BLOB" => row
            .try_get_unchecked::<Vec<u8>, _>(col)
            .map(|b| Value::Bytes(b.into())),
        _ => row.try_get_unchecked::<String, _>(col).map(Value::text),
    };
    decoded.unwrap_or_else(|_| Value::undecodable(class))
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
            .map(|r| (0..r.len()).map(|i| sqlite_cell(r, i)).collect())
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
        assert_eq!(p.rows, vec![vec![Value::Int(4), Value::Null]]);
        let p = fetch_page(&pool, "\"t\"", " WHERE id > 9", 10, 0)
            .await
            .unwrap();
        assert_eq!((p.total, p.columns, p.rows.len()), (0, None, 0));
    }

    #[tokio::test]
    async fn cells_follow_the_value_not_the_declared_type() {
        let pool = SqlitePool::connect("sqlite::memory:").await.unwrap();
        sqlx::query("CREATE TABLE t (a DATETIME, b TEXT, c TEXT, d TEXT, e REAL, f BLOB)")
            .execute(&pool)
            .await
            .unwrap();
        sqlx::query("INSERT INTO t VALUES (1700000000, 'NULL', '', NULL, 1.5, x'0102')")
            .execute(&pool)
            .await
            .unwrap();
        let p = fetch_page(&pool, "\"t\"", "", 10, 0).await.unwrap();
        assert_eq!(
            p.rows,
            vec![vec![
                Value::Int(1_700_000_000),
                Value::text("NULL"),
                Value::text(""),
                Value::Null,
                Value::Float(1.5),
                Value::Bytes(vec![1u8, 2].into()),
            ]]
        );
    }
}
