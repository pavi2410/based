//! Load Postgres tables/views and columns into [`SchemaCache`] for autocomplete.

use std::time::Instant;

use based_core::EngineKind;
use sqlx::{PgPool, Row};

use super::schema_cache::{ColumnInfo, ObjectKind, SchemaCache, SchemaObject};

/// One round-trip: every user-visible column, grouped client-side by relation.
pub async fn load_schema(pool: &PgPool) -> anyhow::Result<SchemaCache> {
    let rows = sqlx::query(
        "SELECT c.table_schema::text AS sch, c.table_name::text AS tbl, \
                t.table_type::text AS kind, c.column_name::text AS col, c.data_type::text AS typ \
         FROM information_schema.columns c \
         JOIN information_schema.tables t \
           ON t.table_schema = c.table_schema AND t.table_name = c.table_name \
         WHERE c.table_schema NOT IN ('pg_catalog', 'information_schema') \
           AND c.table_schema NOT LIKE 'pg_toast%' \
         ORDER BY c.table_schema, c.table_name, c.ordinal_position",
    )
    .fetch_all(pool)
    .await?;

    let mut objects: Vec<SchemaObject> = Vec::new();
    let mut last: Option<(String, String)> = None;
    for row in rows {
        let sch: String = row.try_get("sch")?;
        let tbl: String = row.try_get("tbl")?;
        let key = (sch, tbl);
        if last.as_ref() != Some(&key) {
            let kind: String = row.try_get("kind")?;
            let (sch, tbl) = &key;
            objects.push(SchemaObject {
                full_name: format!("{sch}.{tbl}"),
                label: if sch == "public" {
                    tbl.clone()
                } else {
                    format!("{sch}.{tbl}")
                },
                kind: if kind == "VIEW" {
                    ObjectKind::View
                } else {
                    ObjectKind::Table
                },
                columns: Vec::new(),
            });
            last = Some(key);
        }
        if let Some(obj) = objects.last_mut() {
            obj.columns.push(ColumnInfo {
                name: row.try_get("col")?,
                data_type: row.try_get("typ")?,
            });
        }
    }

    Ok(SchemaCache {
        engine: Some(EngineKind::Postgres),
        objects,
        last_refreshed_at: Some(Instant::now()),
    })
}
