//! Table structure for the inspector: columns, indexes, constraints, stats.

use anyhow::Result;
use based_core::{Grid, TableExtra, TableInfo};
use sqlx::{PgPool, Row, postgres::PgRow};

fn text(row: &PgRow, col: &str) -> String {
    row.try_get::<String, _>(col).unwrap_or_default()
}

fn opt_text(row: &PgRow, col: &str) -> String {
    row.try_get::<Option<String>, _>(col)
        .ok()
        .flatten()
        .unwrap_or_default()
}

pub async fn inspect_table(pool: &PgPool, schema: &str, table: &str) -> Result<TableInfo> {
    let col_rows = sqlx::query(
        r"SELECT ordinal_position, column_name, data_type, is_nullable, column_default
                   FROM information_schema.columns
                   WHERE table_schema = $1 AND table_name = $2
                   ORDER BY ordinal_position",
    )
    .bind(schema)
    .bind(table)
    .fetch_all(pool)
    .await
    .unwrap_or_default();

    let idx_rows = sqlx::query(
        r"SELECT indexname, indexdef FROM pg_indexes
                   WHERE schemaname = $1 AND tablename = $2
                   ORDER BY indexname",
    )
    .bind(schema)
    .bind(table)
    .fetch_all(pool)
    .await
    .unwrap_or_default();

    let con_rows = sqlx::query(
        r"SELECT con.conname AS constraint_name,
                       CASE con.contype
                         WHEN 'p' THEN 'PRIMARY KEY'
                         WHEN 'u' THEN 'UNIQUE'
                         WHEN 'f' THEN 'FOREIGN KEY'
                         WHEN 'c' THEN 'CHECK'
                         ELSE con.contype::text
                       END AS constraint_type,
                       pg_get_constraintdef(con.oid) AS definition
                FROM pg_constraint con
                JOIN pg_class rel ON rel.oid = con.conrelid
                JOIN pg_namespace nsp ON nsp.oid = rel.relnamespace
                WHERE nsp.nspname = $1 AND rel.relname = $2
                ORDER BY con.conname",
    )
    .bind(schema)
    .bind(table)
    .fetch_all(pool)
    .await
    .unwrap_or_default();

    let stat_rows = sqlx::query(
        r"SELECT
                       c.reltuples::bigint AS estimate_rows,
                       pg_size_pretty(pg_relation_size(c.oid)) AS table_size,
                       pg_size_pretty(pg_indexes_size(c.oid)) AS indexes_size,
                       pg_size_pretty(pg_total_relation_size(c.oid)) AS total_size
                FROM pg_class c
                JOIN pg_namespace n ON n.oid = c.relnamespace
                WHERE n.nspname = $1 AND c.relname = $2 AND c.relkind IN ('r', 'p', 'm')",
    )
    .bind(schema)
    .bind(table)
    .fetch_optional(pool)
    .await
    .unwrap_or(None);

    let columns = Grid::new(
        &["#", "Column", "Type", "NULL", "Default"],
        col_rows
            .iter()
            .map(|r| {
                vec![
                    r.try_get::<i32, _>("ordinal_position")
                        .unwrap_or(0)
                        .to_string(),
                    text(r, "column_name"),
                    text(r, "data_type"),
                    text(r, "is_nullable"),
                    opt_text(r, "column_default"),
                ]
            })
            .collect(),
    );
    let indexes = Grid::new(
        &["Index", "Definition"],
        idx_rows
            .iter()
            .map(|r| vec![text(r, "indexname"), text(r, "indexdef")])
            .collect(),
    );
    let constraints = Grid::new(
        &["Constraint", "Type", "Definition"],
        con_rows
            .iter()
            .map(|r| {
                vec![
                    text(r, "constraint_name"),
                    text(r, "constraint_type"),
                    text(r, "definition"),
                ]
            })
            .collect(),
    );
    let stats = match stat_rows.as_ref() {
        Some(sr) => {
            let estimate = sr
                .try_get::<i64, _>("estimate_rows")
                .map_or_else(|_| "—".to_string(), |n| n.to_string());
            vec![
                ("Estimated rows".into(), estimate),
                ("Table size".into(), opt_text(sr, "table_size")),
                ("Indexes size".into(), opt_text(sr, "indexes_size")),
                ("Total size".into(), opt_text(sr, "total_size")),
            ]
        }
        None => vec![(
            "(no stats)".into(),
            "relation not found or not a regular table".into(),
        )],
    };
    Ok(TableInfo {
        columns,
        indexes,
        constraints,
        extra: TableExtra::Stats(stats),
    })
}
