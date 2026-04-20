//! SQLite-specific engine operations: object listing, table browse,
//! raw SQL execution, and BLOB/value→JSON conversion.

use crate::error::Error;
use serde::{Deserialize, Serialize};
use specta::Type;
use sqlx::{Column, QueryBuilder, Row, Sqlite, SqlitePool, TypeInfo, ValueRef};

use super::filters::{parse_filters, push_sql_order, push_sql_where, quote_ident};
use super::types::{
    BrowseOptions, ColumnDescription, ColumnInfo, ForeignKeyDescription, IndexDescription,
    QueryResult, TableDescription,
};
use super::values::{blob_marker, f64_to_json};

/// User-visible SQLite object (table, view, index...).
#[derive(Debug, Serialize, Deserialize, Type)]
pub struct SQLiteObject {
    pub name: String,
}

pub async fn list_objects(
    pool: &SqlitePool,
    object_type: &str,
) -> Result<Vec<SQLiteObject>, Error> {
    let rows = sqlx::query("SELECT name FROM sqlite_schema WHERE type = ? ORDER BY name")
        .bind(object_type)
        .fetch_all(pool)
        .await?;

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
    // `PRAGMA` doesn't accept bind parameters, so we single-quote the
    // table name and double any embedded `'`. Identifier quoting with
    // `"..."` is not accepted by `PRAGMA table_info`.
    let pragma_sql = format!("PRAGMA table_info('{}')", table_name.replace('\'', "''"));
    let column_rows = sqlx::query(&pragma_sql).fetch_all(pool).await?;

    let columns: Vec<ColumnInfo> = column_rows
        .iter()
        .map(|row| ColumnInfo {
            name: row.get::<String, _>("name"),
            data_type: row.get::<String, _>("type"),
        })
        .collect();

    let filters = parse_filters(options.filters.as_deref());
    let quoted_table = quote_ident(table_name);

    let mut count_qb: QueryBuilder<Sqlite> = QueryBuilder::new("SELECT COUNT(*) as cnt FROM ");
    count_qb.push(&quoted_table);
    push_sql_where(&mut count_qb, &filters);
    let total_count: i64 = count_qb.build().fetch_one(pool).await?.get("cnt");

    let mut data_qb: QueryBuilder<Sqlite> = QueryBuilder::new("SELECT * FROM ");
    data_qb.push(&quoted_table);
    push_sql_where(&mut data_qb, &filters);
    push_sql_order(
        &mut data_qb,
        &options.order_by_column,
        &options.order_by_direction,
    );
    data_qb.push(" LIMIT ");
    data_qb.push_bind(options.limit.unwrap_or(100));
    data_qb.push(" OFFSET ");
    data_qb.push_bind(options.offset.unwrap_or(0));

    let data_rows = data_qb.build().fetch_all(pool).await?;

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

/// Describe a SQLite table using the `pragma_*` table-valued functions
/// (available since SQLite 3.16). This is a single round-trip per
/// catalog (columns, indexes, foreign keys).
pub async fn describe_table(
    pool: &SqlitePool,
    table_name: &str,
) -> Result<TableDescription, Error> {
    // Whether the object is a table or a view.
    let kind: String = sqlx::query_scalar(
        "SELECT type FROM sqlite_schema WHERE name = ? AND type IN ('table','view')",
    )
    .bind(table_name)
    .fetch_optional(pool)
    .await?
    .unwrap_or_else(|| "table".to_string());

    let column_rows = sqlx::query(
        "SELECT cid, name, type, \"notnull\", dflt_value, pk \
         FROM pragma_table_info(?) ORDER BY cid",
    )
    .bind(table_name)
    .fetch_all(pool)
    .await?;

    let columns: Vec<ColumnDescription> = column_rows
        .iter()
        .map(|row| {
            let pk: i32 = row.get("pk");
            let notnull: i32 = row.get("notnull");
            let cid: i32 = row.get("cid");
            ColumnDescription {
                name: row.get("name"),
                data_type: row.get("type"),
                nullable: notnull == 0,
                default: row
                    .try_get::<Option<String>, _>("dflt_value")
                    .unwrap_or(None),
                is_primary_key: pk > 0,
                position: cid + 1,
            }
        })
        .collect();

    // Indexes, then the columns in each index.
    let index_rows = sqlx::query(
        "SELECT name, \"unique\" as uniq, origin FROM pragma_index_list(?) ORDER BY seq",
    )
    .bind(table_name)
    .fetch_all(pool)
    .await?;

    let mut indexes: Vec<IndexDescription> = Vec::new();
    for row in index_rows {
        let name: String = row.get("name");
        let uniq: i32 = row.get("uniq");
        let origin: String = row.get("origin");
        let col_rows = sqlx::query("SELECT name FROM pragma_index_info(?) ORDER BY seqno")
            .bind(&name)
            .fetch_all(pool)
            .await?;
        let cols: Vec<String> = col_rows.iter().map(|r| r.get("name")).collect();
        indexes.push(IndexDescription {
            name,
            columns: cols,
            unique: uniq != 0,
            primary: origin == "pk",
        });
    }

    let fk_rows = sqlx::query(
        "SELECT id, seq, \"table\" as ref_table, \"from\" as from_col, \"to\" as to_col \
         FROM pragma_foreign_key_list(?) ORDER BY id, seq",
    )
    .bind(table_name)
    .fetch_all(pool)
    .await?;

    // Group foreign key rows by `id`, preserving insertion order.
    let mut fks: Vec<ForeignKeyDescription> = Vec::new();
    let mut current_id: Option<i32> = None;
    for row in fk_rows {
        let id: i32 = row.get("id");
        let from_col: String = row.get("from_col");
        let to_col: String = row.get("to_col");
        let ref_table: String = row.get("ref_table");
        if Some(id) != current_id {
            current_id = Some(id);
            fks.push(ForeignKeyDescription {
                name: None,
                columns: vec![from_col],
                referenced_schema: None,
                referenced_table: ref_table,
                referenced_columns: vec![to_col],
            });
        } else if let Some(last) = fks.last_mut() {
            last.columns.push(from_col);
            last.referenced_columns.push(to_col);
        }
    }

    Ok(TableDescription {
        name: table_name.to_string(),
        schema: None,
        kind,
        columns,
        indexes,
        foreign_keys: fks,
        row_count: None,
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
                f64_to_json(v)
            } else if let Ok(v) = row.try_get::<String, _>(column) {
                serde_json::Value::String(v)
            } else if let Ok(v) = row.try_get::<bool, _>(column) {
                serde_json::Value::Bool(v)
            } else if let Ok(v) = row.try_get::<Vec<u8>, _>(column) {
                blob_marker(v.len())
            } else {
                serde_json::Value::Null
            }
        }
        Err(_) => serde_json::Value::Null,
    }
}
