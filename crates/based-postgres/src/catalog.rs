//! Per-column catalog metadata (types, keys) for table browse.

use std::collections::HashMap;

use anyhow::Result;
use based_core::{ColumnCatalog, ColumnMeta, FkTarget, ObjectRef};
use sqlx::{PgPool, Row};

pub async fn column_catalog(pool: &PgPool, schema: &str, table: &str) -> Result<ColumnCatalog> {
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
                WHEN c.data_type = 'ARRAY' THEN ltrim(c.udt_name, '_') || '[]'
                ELSE c.data_type
            END AS display_type,
            c.is_nullable = 'YES' AS nullable,
            COALESCE(bool_or(tc.constraint_type = 'PRIMARY KEY'), false) AS is_pk,
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
                 c.numeric_precision, c.numeric_scale, c.datetime_precision, c.is_nullable,
                 c.udt_name
        ORDER BY c.ordinal_position",
    )
    .bind(schema)
    .bind(table)
    .fetch_all(pool)
    .await?;

    // From pg_constraint, not information_schema.constraint_column_usage: that
    // view only lists referenced tables owned by an enabled role, and joining it
    // pairs every local column of a composite key with every referenced one.
    // `unnest(conkey, confkey)` pairs them by position.
    let fk_rows = sqlx::query(
        r"SELECT
            a.attname AS column_name,
            rn.nspname AS ref_schema,
            rc.relname AS ref_table,
            ra.attname AS ref_column
        FROM pg_constraint con
        JOIN pg_class c ON c.oid = con.conrelid
        JOIN pg_namespace n ON n.oid = c.relnamespace
        CROSS JOIN LATERAL unnest(con.conkey, con.confkey) AS k(attnum, ref_attnum)
        JOIN pg_attribute a ON a.attrelid = con.conrelid AND a.attnum = k.attnum
        JOIN pg_class rc ON rc.oid = con.confrelid
        JOIN pg_namespace rn ON rn.oid = rc.relnamespace
        JOIN pg_attribute ra ON ra.attrelid = con.confrelid AND ra.attnum = k.ref_attnum
        WHERE con.contype = 'f' AND n.nspname = $1 AND c.relname = $2
        ORDER BY con.conname, k.attnum",
    )
    .bind(schema)
    .bind(table)
    .fetch_all(pool)
    .await?;

    let mut fk_targets: HashMap<String, FkTarget> = HashMap::new();
    for row in fk_rows {
        let col: String = row.try_get("column_name")?;
        let ref_schema: String = row.try_get("ref_schema")?;
        let ref_table: String = row.try_get("ref_table")?;
        let ref_column: String = row.try_get("ref_column")?;
        // A column in two foreign keys: keep the first (by constraint name).
        fk_targets.entry(col).or_insert(FkTarget {
            table: ObjectRef::qualified(ref_schema, ref_table),
            column: ref_column,
        });
    }

    let mut out = ColumnCatalog::default();
    for row in col_rows {
        let name: String = row.try_get("column_name")?;
        let display_type: String = row.try_get("display_type")?;
        let nullable: bool = row.try_get("nullable")?;
        let is_pk: bool = row.try_get("is_pk")?;
        let is_unique: bool = row.try_get("is_unique")?;
        out.push(
            name.clone(),
            ColumnMeta {
                data_type: Some(display_type),
                nullable: Some(nullable),
                is_primary_key: is_pk,
                is_unique,
                fk: fk_targets.remove(&name),
            },
        );
    }
    Ok(out)
}
