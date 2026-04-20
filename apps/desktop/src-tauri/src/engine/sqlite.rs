//! SQLite-specific engine operations: object listing, table browse,
//! raw SQL execution, and BLOB/value→JSON conversion.

use crate::error::Error;
use serde::{Deserialize, Serialize};
use specta::Type;
use sqlx::{Column, Row, SqlitePool, TypeInfo, ValueRef};

use super::filters::{build_sql_where_clause, order_clause_sql, parse_filters};
use super::types::{BrowseOptions, ColumnInfo, QueryResult};

/// User-visible SQLite object (table, view, index...).
#[derive(Debug, Serialize, Deserialize, Type)]
pub struct SQLiteObject {
    pub name: String,
}

pub async fn list_objects(
    pool: &SqlitePool,
    object_type: &str,
) -> Result<Vec<SQLiteObject>, Error> {
    // `type` only ever takes values from a small static set emitted by
    // the frontend (table / view / index / trigger); interpolating into
    // the query is safe but the eventual `FilterAst` refactor will
    // switch this to a bound parameter.
    let query = format!(
        "SELECT name FROM sqlite_schema WHERE type = '{}' ORDER BY name",
        object_type
    );

    let rows = sqlx::query(&query).fetch_all(pool).await?;

    Ok(rows
        .iter()
        .map(|row| SQLiteObject { name: row.get(0) })
        .collect())
}

pub async fn browse_table(
    pool: &SqlitePool,
    table_name: &str,
    options: &BrowseOptions,
) -> Result<QueryResult, Error> {
    let column_rows = sqlx::query(&format!("PRAGMA table_info('{}')", table_name))
        .fetch_all(pool)
        .await?;

    let columns: Vec<ColumnInfo> = column_rows
        .iter()
        .map(|row| ColumnInfo {
            name: row.get::<String, _>("name"),
            data_type: row.get::<String, _>("type"),
        })
        .collect();

    let filters = parse_filters(options.filters.as_deref());
    let where_clause = build_sql_where_clause(&filters);
    let order_clause = order_clause_sql(&options.order_by_column, &options.order_by_direction);

    let count_query = format!(
        "SELECT COUNT(*) as cnt FROM \"{}\"{}",
        table_name, where_clause
    );
    let count_row = sqlx::query(&count_query).fetch_one(pool).await?;
    let total_count: i64 = count_row.get("cnt");

    let limit = options.limit.unwrap_or(100);
    let offset = options.offset.unwrap_or(0);
    let data_query = format!(
        "SELECT * FROM \"{}\"{}{} LIMIT {} OFFSET {}",
        table_name, where_clause, order_clause, limit, offset
    );

    let data_rows = sqlx::query(&data_query).fetch_all(pool).await?;

    let rows: Vec<Vec<serde_json::Value>> = data_rows
        .iter()
        .map(|row| {
            columns
                .iter()
                .map(|col| value_to_json(row, &col.name))
                .collect()
        })
        .collect();

    Ok(QueryResult {
        columns,
        rows,
        total_count: Some(total_count),
    })
}

pub async fn execute_raw(pool: &SqlitePool, query: &str) -> Result<QueryResult, Error> {
    let rows = sqlx::query(query).fetch_all(pool).await?;

    if rows.is_empty() {
        return Ok(QueryResult {
            columns: vec![],
            rows: vec![],
            total_count: Some(0),
        });
    }

    let columns: Vec<ColumnInfo> = rows[0]
        .columns()
        .iter()
        .map(|col| ColumnInfo {
            name: col.name().to_string(),
            data_type: col.type_info().name().to_string(),
        })
        .collect();

    let result_rows: Vec<Vec<serde_json::Value>> = rows
        .iter()
        .map(|row| {
            columns
                .iter()
                .map(|col| value_to_json(row, &col.name))
                .collect()
        })
        .collect();

    let row_count = result_rows.len() as i64;
    Ok(QueryResult {
        columns,
        rows: result_rows,
        total_count: Some(row_count),
    })
}

/// Convert a SQLite row cell to JSON, trying the common scalar types in
/// order. Binary blobs are surfaced as a size marker rather than a
/// (potentially huge / invalid-utf8) string.
pub fn value_to_json(row: &sqlx::sqlite::SqliteRow, column: &str) -> serde_json::Value {
    let value_ref = row.try_get_raw(column);
    match value_ref {
        Ok(val) if val.is_null() => serde_json::Value::Null,
        Ok(_) => {
            if let Ok(v) = row.try_get::<i64, _>(column) {
                serde_json::Value::Number(v.into())
            } else if let Ok(v) = row.try_get::<f64, _>(column) {
                serde_json::Number::from_f64(v)
                    .map(serde_json::Value::Number)
                    .unwrap_or(serde_json::Value::Null)
            } else if let Ok(v) = row.try_get::<String, _>(column) {
                serde_json::Value::String(v)
            } else if let Ok(v) = row.try_get::<bool, _>(column) {
                serde_json::Value::Bool(v)
            } else if let Ok(v) = row.try_get::<Vec<u8>, _>(column) {
                serde_json::Value::String(format!("[BLOB: {} bytes]", v.len()))
            } else {
                serde_json::Value::Null
            }
        }
        Err(_) => serde_json::Value::Null,
    }
}
