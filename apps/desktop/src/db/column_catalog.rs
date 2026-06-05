//! Per-column catalog metadata for SQL table browse grids.

use std::collections::{HashMap, HashSet};

use anyhow::Result;
use sqlx::{AssertSqlSafe, PgPool, Row, SqlitePool};

use crate::widgets::column_header::GridColumnMeta;

pub async fn load_postgres_column_catalog(
    pool: &PgPool,
    schema: &str,
    table: &str,
) -> Result<HashMap<String, GridColumnMeta>> {
    let col_rows = sqlx::query(
        r"SELECT
            c.column_name,
            CASE
                WHEN c.character_maximum_length IS NOT NULL THEN
                    c.data_type || '(' || c.character_maximum_length::text || ')'
                WHEN c.numeric_precision IS NOT NULL AND c.numeric_scale IS NOT NULL THEN
                    c.data_type || '(' || c.numeric_precision::text || ',' || c.numeric_scale::text || ')'
                WHEN c.datetime_precision IS NOT NULL
                    AND c.data_type IN ('timestamp without time zone', 'timestamp with time zone', 'time without time zone', 'time with time zone')
                THEN c.data_type || '(' || c.datetime_precision::text || ')'
                ELSE c.data_type
            END AS display_type,
            c.is_nullable = 'YES' AS nullable,
            COALESCE(bool_or(tc.constraint_type = 'PRIMARY KEY'), false) AS is_pk,
            COALESCE(bool_or(tc.constraint_type = 'FOREIGN KEY'), false) AS is_fk,
            COALESCE(bool_or(tc.constraint_type = 'UNIQUE'), false) AS is_unique
        FROM information_schema.columns c
        LEFT JOIN information_schema.key_column_usage kcu
            ON c.table_schema = kcu.table_schema
            AND c.table_name = kcu.table_name
            AND c.column_name = kcu.column_name
        LEFT JOIN information_schema.table_constraints tc
            ON kcu.constraint_schema = tc.constraint_schema
            AND kcu.constraint_name = tc.constraint_name
            AND tc.table_schema = c.table_schema
            AND tc.table_name = c.table_name
        WHERE c.table_schema = $1 AND c.table_name = $2
        GROUP BY c.column_name, c.ordinal_position, c.data_type, c.character_maximum_length,
                 c.numeric_precision, c.numeric_scale, c.datetime_precision, c.is_nullable
        ORDER BY c.ordinal_position",
    )
    .bind(schema)
    .bind(table)
    .fetch_all(pool)
    .await?;

    let fk_rows = sqlx::query(
        r"SELECT
            kcu.column_name,
            ccu.table_schema AS ref_schema,
            ccu.table_name AS ref_table,
            ccu.column_name AS ref_column
        FROM information_schema.table_constraints tc
        JOIN information_schema.key_column_usage kcu
            ON tc.constraint_schema = kcu.constraint_schema
            AND tc.constraint_name = kcu.constraint_name
            AND tc.table_schema = kcu.table_schema
            AND tc.table_name = kcu.table_name
        JOIN information_schema.constraint_column_usage ccu
            ON ccu.constraint_schema = tc.constraint_schema
            AND ccu.constraint_name = tc.constraint_name
        WHERE tc.constraint_type = 'FOREIGN KEY'
            AND tc.table_schema = $1
            AND tc.table_name = $2",
    )
    .bind(schema)
    .bind(table)
    .fetch_all(pool)
    .await?;

    let mut fk_targets: HashMap<String, String> = HashMap::new();
    for row in fk_rows {
        let col: String = row.try_get("column_name")?;
        let ref_schema: String = row.try_get("ref_schema")?;
        let ref_table: String = row.try_get("ref_table")?;
        let ref_column: String = row.try_get("ref_column")?;
        fk_targets.insert(col, format!("{ref_schema}.{ref_table}({ref_column})"));
    }

    let mut out = HashMap::new();
    for row in col_rows {
        let name: String = row.try_get("column_name")?;
        let display_type: String = row.try_get("display_type")?;
        let nullable: bool = row.try_get("nullable")?;
        let is_pk: bool = row.try_get("is_pk")?;
        let is_fk: bool = row.try_get("is_fk")?;
        let is_unique: bool = row.try_get("is_unique")?;
        out.insert(
            name.clone(),
            GridColumnMeta {
                data_type: Some(display_type),
                nullable: Some(nullable),
                is_primary_key: is_pk,
                is_foreign_key: is_fk,
                is_unique,
                fk_target: fk_targets.get(&name).cloned(),
            },
        );
    }
    Ok(out)
}

pub async fn load_sqlite_column_catalog(
    pool: &SqlitePool,
    table: &str,
) -> Result<HashMap<String, GridColumnMeta>> {
    let col_sql = format!("PRAGMA table_info(\"{table}\")");
    let col_rows = sqlx::query(AssertSqlSafe(col_sql)).fetch_all(pool).await?;

    let mut out = HashMap::new();
    let mut pk_columns = HashSet::new();
    for row in &col_rows {
        let name: String = row.try_get("name")?;
        let type_name: String = row.try_get("type")?;
        let notnull: bool = row.try_get("notnull")?;
        let pk: i64 = row.try_get("pk")?;
        if pk > 0 {
            pk_columns.insert(name.clone());
        }
        out.insert(
            name.clone(),
            GridColumnMeta {
                data_type: if type_name.is_empty() {
                    None
                } else {
                    Some(type_name)
                },
                nullable: Some(!notnull),
                is_primary_key: pk > 0,
                ..Default::default()
            },
        );
    }

    let fk_sql = format!("PRAGMA foreign_key_list(\"{table}\")");
    let fk_rows = sqlx::query(AssertSqlSafe(fk_sql)).fetch_all(pool).await?;
    for row in fk_rows {
        let from_col: String = row.try_get("from")?;
        let to_table: String = row.try_get("table")?;
        let to_col: String = row.try_get("to")?;
        if let Some(meta) = out.get_mut(&from_col) {
            meta.is_foreign_key = true;
            meta.fk_target = Some(format!("{to_table}({to_col})"));
        }
    }

    let idx_sql = format!("PRAGMA index_list(\"{table}\")");
    let idx_rows = sqlx::query(AssertSqlSafe(idx_sql)).fetch_all(pool).await?;
    for idx in idx_rows {
        let unique: bool = idx.try_get("unique")?;
        if !unique {
            continue;
        }
        let idx_name: String = idx.try_get("name")?;
        if idx_name.starts_with("sqlite_autoindex") {
            continue;
        }
        let info_sql = format!("PRAGMA index_info(\"{idx_name}\")");
        let info_rows = sqlx::query(AssertSqlSafe(info_sql)).fetch_all(pool).await?;
        for info in info_rows {
            let col_name: String = info.try_get("name")?;
            if pk_columns.contains(&col_name) {
                continue;
            }
            if let Some(meta) = out.get_mut(&col_name) {
                meta.is_unique = true;
            }
        }
    }

    Ok(out)
}
