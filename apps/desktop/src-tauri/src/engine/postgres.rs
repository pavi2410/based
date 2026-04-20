//! PostgreSQL-specific engine operations: schema/table listing, table
//! browse, raw SQL execution, and value→JSON conversion.

use crate::error::Error;
use serde::{Deserialize, Serialize};
use specta::Type;
use sqlx::{Column, PgPool, Postgres, QueryBuilder, Row, TypeInfo, ValueRef};

use super::filters::{parse_filters, push_sql_order, push_sql_where, quote_ident};
use super::types::{
    BrowseOptions, ColumnDescription, ColumnInfo, ForeignKeyDescription, IndexDescription,
    QueryResult, TableDescription,
};
use super::values::f64_to_json;

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
    let qualified = format!("{}.{}", quote_ident(schema), quote_ident(table_name));

    let mut count_qb: QueryBuilder<Postgres> = QueryBuilder::new("SELECT COUNT(*) as cnt FROM ");
    count_qb.push(&qualified);
    push_sql_where(&mut count_qb, &filters);
    let total_count: i64 = count_qb.build().fetch_one(pool).await?.get("cnt");

    let mut data_qb: QueryBuilder<Postgres> = QueryBuilder::new("SELECT * FROM ");
    data_qb.push(&qualified);
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

/// Describe a PostgreSQL table: columns, indexes, and foreign keys.
/// Uses `information_schema` + `pg_catalog`; all predicates are bound
/// to prevent identifier injection.
pub async fn describe_table(
    pool: &PgPool,
    schema: &str,
    table_name: &str,
) -> Result<TableDescription, Error> {
    let kind: String = sqlx::query_scalar(
        "SELECT CASE WHEN table_type = 'BASE TABLE' THEN 'table' \
                     WHEN table_type = 'VIEW' THEN 'view' \
                     ELSE lower(table_type) END \
         FROM information_schema.tables \
         WHERE table_schema = $1 AND table_name = $2",
    )
    .bind(schema)
    .bind(table_name)
    .fetch_optional(pool)
    .await?
    .unwrap_or_else(|| "table".to_string());

    // Columns, including primary-key membership from the
    // information_schema key-column-usage / table-constraints join.
    let column_rows = sqlx::query(
        r#"
        SELECT
            c.column_name,
            c.data_type,
            c.is_nullable,
            c.column_default,
            c.ordinal_position,
            COALESCE(pk.is_pk, false) AS is_pk
        FROM information_schema.columns c
        LEFT JOIN (
            SELECT kcu.column_name, true AS is_pk
            FROM information_schema.table_constraints tc
            JOIN information_schema.key_column_usage kcu
              ON tc.constraint_name = kcu.constraint_name
             AND tc.table_schema = kcu.table_schema
             AND tc.table_name = kcu.table_name
            WHERE tc.constraint_type = 'PRIMARY KEY'
              AND tc.table_schema = $1
              AND tc.table_name = $2
        ) pk ON pk.column_name = c.column_name
        WHERE c.table_schema = $1 AND c.table_name = $2
        ORDER BY c.ordinal_position
        "#,
    )
    .bind(schema)
    .bind(table_name)
    .fetch_all(pool)
    .await?;

    let columns: Vec<ColumnDescription> = column_rows
        .iter()
        .map(|row| {
            let nullable: String = row.get("is_nullable");
            let position: i32 = row.get::<i32, _>("ordinal_position");
            ColumnDescription {
                name: row.get("column_name"),
                data_type: row.get("data_type"),
                nullable: nullable == "YES",
                default: row.try_get::<Option<String>, _>("column_default").unwrap_or(None),
                is_primary_key: row.get("is_pk"),
                position,
            }
        })
        .collect();

    // Indexes via pg_index; one row per index, columns aggregated.
    let index_rows = sqlx::query(
        r#"
        SELECT
            i.relname                                             AS index_name,
            ix.indisunique                                        AS is_unique,
            ix.indisprimary                                       AS is_primary,
            ARRAY(
                SELECT a.attname
                FROM pg_attribute a
                WHERE a.attrelid = t.oid
                  AND a.attnum = ANY(ix.indkey)
                ORDER BY array_position(ix.indkey, a.attnum)
            )                                                     AS column_names
        FROM pg_class t
        JOIN pg_index ix           ON t.oid = ix.indrelid
        JOIN pg_class i            ON i.oid = ix.indexrelid
        JOIN pg_namespace n        ON n.oid = t.relnamespace
        WHERE n.nspname = $1 AND t.relname = $2
        ORDER BY i.relname
        "#,
    )
    .bind(schema)
    .bind(table_name)
    .fetch_all(pool)
    .await?;

    let indexes: Vec<IndexDescription> = index_rows
        .iter()
        .map(|row| IndexDescription {
            name: row.get("index_name"),
            columns: row
                .try_get::<Vec<String>, _>("column_names")
                .unwrap_or_default(),
            unique: row.get("is_unique"),
            primary: row.get("is_primary"),
        })
        .collect();

    // Foreign keys: aggregate referencing/referenced columns per
    // constraint so multi-column FKs come out as a single row.
    let fk_rows = sqlx::query(
        r#"
        SELECT
            tc.constraint_name                                         AS name,
            ccu.table_schema                                           AS ref_schema,
            ccu.table_name                                             AS ref_table,
            ARRAY_AGG(kcu.column_name  ORDER BY kcu.ordinal_position)  AS columns,
            ARRAY_AGG(ccu.column_name  ORDER BY kcu.ordinal_position)  AS ref_columns
        FROM information_schema.table_constraints tc
        JOIN information_schema.key_column_usage kcu
          ON tc.constraint_name = kcu.constraint_name
         AND tc.table_schema = kcu.table_schema
        JOIN information_schema.constraint_column_usage ccu
          ON ccu.constraint_name = tc.constraint_name
         AND ccu.table_schema = tc.table_schema
        WHERE tc.constraint_type = 'FOREIGN KEY'
          AND tc.table_schema = $1
          AND tc.table_name   = $2
        GROUP BY tc.constraint_name, ccu.table_schema, ccu.table_name
        ORDER BY tc.constraint_name
        "#,
    )
    .bind(schema)
    .bind(table_name)
    .fetch_all(pool)
    .await?;

    let foreign_keys: Vec<ForeignKeyDescription> = fk_rows
        .iter()
        .map(|row| ForeignKeyDescription {
            name: row.try_get::<String, _>("name").ok(),
            columns: row.try_get::<Vec<String>, _>("columns").unwrap_or_default(),
            referenced_schema: row.try_get::<String, _>("ref_schema").ok(),
            referenced_table: row.get("ref_table"),
            referenced_columns: row
                .try_get::<Vec<String>, _>("ref_columns")
                .unwrap_or_default(),
        })
        .collect();

    // Postgres exposes an estimated row count via pg_class.reltuples;
    // treat negative / stale values as "unknown" rather than misleading.
    let est_rows: Option<f64> = sqlx::query_scalar(
        "SELECT c.reltuples::float8 FROM pg_class c \
         JOIN pg_namespace n ON n.oid = c.relnamespace \
         WHERE n.nspname = $1 AND c.relname = $2",
    )
    .bind(schema)
    .bind(table_name)
    .fetch_optional(pool)
    .await?;

    let row_count = est_rows.and_then(|v| {
        if v.is_finite() && v >= 0.0 {
            Some(v as i64)
        } else {
            None
        }
    });

    Ok(TableDescription {
        name: table_name.to_string(),
        schema: Some(schema.to_string()),
        kind,
        columns,
        indexes,
        foreign_keys,
        row_count,
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
                f64_to_json(v)
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
