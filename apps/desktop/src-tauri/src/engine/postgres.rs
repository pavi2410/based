//! PostgreSQL-specific engine operations: schema/table listing, table
//! browse, raw SQL execution, and value→JSON conversion.

use crate::error::Error;
use serde::{Deserialize, Serialize};
use specta::Type;
use sqlx::{Column, PgPool, Row, TypeInfo, ValueRef};

use super::filters::{build_sql_where_clause, order_clause_sql, parse_filters};
use super::types::{BrowseOptions, ColumnInfo, QueryResult};

#[derive(Debug, Serialize, Deserialize, Type)]
pub struct PostgresSchema {
    pub name: String,
}

#[derive(Debug, Serialize, Deserialize, Type)]
pub struct PostgresTable {
    pub name: String,
    pub schema: String,
}

pub async fn list_schemas(pool: &PgPool) -> Result<Vec<PostgresSchema>, Error> {
    let query = "SELECT schema_name as name FROM information_schema.schemata
                 WHERE schema_name NOT IN ('pg_catalog', 'information_schema', 'pg_toast')
                 ORDER BY schema_name";
    let rows = sqlx::query(query).fetch_all(pool).await?;
    Ok(rows
        .iter()
        .map(|row| PostgresSchema { name: row.get(0) })
        .collect())
}

pub async fn list_tables(pool: &PgPool, schema: &str) -> Result<Vec<PostgresTable>, Error> {
    let query = "SELECT table_name as name, table_schema as schema
                 FROM information_schema.tables
                 WHERE table_schema = $1 AND table_type = 'BASE TABLE'
                 ORDER BY table_name";

    let rows = sqlx::query(query).bind(schema).fetch_all(pool).await?;

    Ok(rows
        .iter()
        .map(|row| PostgresTable {
            name: row.get(0),
            schema: row.get(1),
        })
        .collect())
}

pub async fn browse_table(
    pool: &PgPool,
    schema: &str,
    table_name: &str,
    options: &BrowseOptions,
) -> Result<QueryResult, Error> {
    let column_query = r#"
        SELECT column_name, data_type
        FROM information_schema.columns
        WHERE table_schema = $1 AND table_name = $2
        ORDER BY ordinal_position
    "#;
    let column_rows = sqlx::query(column_query)
        .bind(schema)
        .bind(table_name)
        .fetch_all(pool)
        .await?;

    let columns: Vec<ColumnInfo> = column_rows
        .iter()
        .map(|row| ColumnInfo {
            name: row.get("column_name"),
            data_type: row.get("data_type"),
        })
        .collect();

    let filters = parse_filters(options.filters.as_deref());
    let where_clause = build_sql_where_clause(&filters);
    let order_clause = order_clause_sql(&options.order_by_column, &options.order_by_direction);

    let count_query = format!(
        "SELECT COUNT(*) as cnt FROM \"{}\".\"{}\"{}",
        schema, table_name, where_clause
    );
    let count_row = sqlx::query(&count_query).fetch_one(pool).await?;
    let total_count: i64 = count_row.get("cnt");

    let limit = options.limit.unwrap_or(100);
    let offset = options.offset.unwrap_or(0);
    let data_query = format!(
        "SELECT * FROM \"{}\".\"{}\"{}{} LIMIT {} OFFSET {}",
        schema, table_name, where_clause, order_clause, limit, offset
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

pub async fn execute_raw(pool: &PgPool, query: &str) -> Result<QueryResult, Error> {
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

/// Convert a PostgreSQL row cell to JSON. Tries the common scalar types
/// in order, including chrono date/time types and native `jsonb`, and
/// falls back to a string representation.
pub fn value_to_json(row: &sqlx::postgres::PgRow, column: &str) -> serde_json::Value {
    let value_ref = row.try_get_raw(column);
    match value_ref {
        Ok(val) if val.is_null() => serde_json::Value::Null,
        Ok(_) => {
            if let Ok(v) = row.try_get::<i64, _>(column) {
                serde_json::Value::Number(v.into())
            } else if let Ok(v) = row.try_get::<i32, _>(column) {
                serde_json::Value::Number(v.into())
            } else if let Ok(v) = row.try_get::<f64, _>(column) {
                serde_json::Number::from_f64(v)
                    .map(serde_json::Value::Number)
                    .unwrap_or(serde_json::Value::Null)
            } else if let Ok(v) = row.try_get::<String, _>(column) {
                serde_json::Value::String(v)
            } else if let Ok(v) = row.try_get::<bool, _>(column) {
                serde_json::Value::Bool(v)
            } else if let Ok(v) = row.try_get::<chrono::NaiveDateTime, _>(column) {
                serde_json::Value::String(v.to_string())
            } else if let Ok(v) = row.try_get::<chrono::DateTime<chrono::Utc>, _>(column) {
                serde_json::Value::String(v.to_rfc3339())
            } else if let Ok(v) = row.try_get::<chrono::NaiveDate, _>(column) {
                serde_json::Value::String(v.to_string())
            } else if let Ok(v) = row.try_get::<serde_json::Value, _>(column) {
                v
            } else {
                row.try_get::<String, _>(column)
                    .map(serde_json::Value::String)
                    .unwrap_or(serde_json::Value::Null)
            }
        }
        Err(_) => serde_json::Value::Null,
    }
}
