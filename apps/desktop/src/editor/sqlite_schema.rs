//! Load SQLite schema objects into [`SchemaCache`] for editor autocomplete.

use std::time::Instant;

use based_core::EngineKind;
use sqlx::{AssertSqlSafe, Row, SqlitePool};

use super::schema_cache::{ColumnInfo, ObjectKind, SchemaCache, SchemaObject};

/// Fetch tables/views and column metadata from a connected SQLite pool.
pub async fn load_schema(pool: &SqlitePool) -> anyhow::Result<SchemaCache> {
    let rows = sqlx::query(
        "SELECT name, type FROM sqlite_master \
         WHERE type IN ('table','view') AND name NOT LIKE 'sqlite_%' \
         ORDER BY name",
    )
    .fetch_all(pool)
    .await?;

    let mut objects = Vec::with_capacity(rows.len());
    for row in rows {
        let name: String = row.try_get("name")?;
        let type_str: String = row.try_get("type")?;
        let kind = sqlite_object_kind(&type_str);
        let columns = load_columns(pool, &name).await?;
        objects.push(SchemaObject {
            full_name: name.clone(),
            label: name,
            kind,
            columns,
        });
    }

    Ok(SchemaCache {
        engine: Some(EngineKind::SQLite),
        objects,
        last_refreshed_at: Some(Instant::now()),
    })
}

async fn load_columns(pool: &SqlitePool, table_name: &str) -> anyhow::Result<Vec<ColumnInfo>> {
    let sql = format!("PRAGMA table_info(\"{table_name}\")");
    let rows = sqlx::query(AssertSqlSafe(sql)).fetch_all(pool).await?;
    Ok(rows
        .iter()
        .map(|row| ColumnInfo {
            name: row.try_get::<String, _>("name").unwrap_or_default(),
            data_type: row.try_get::<String, _>("type").unwrap_or_default(),
            nullable: !row.try_get::<bool, _>("notnull").unwrap_or(false),
            is_primary_key: row.try_get::<i64, _>("pk").unwrap_or(0) != 0,
        })
        .collect())
}

fn sqlite_object_kind(type_str: &str) -> ObjectKind {
    match type_str {
        "view" => ObjectKind::View,
        _ => ObjectKind::Table,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn maps_view_kind() {
        assert_eq!(sqlite_object_kind("view"), ObjectKind::View);
        assert_eq!(sqlite_object_kind("table"), ObjectKind::Table);
    }
}
