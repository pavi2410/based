//! The objects a connection lists: tables, views and triggers.

use anyhow::Result;
use based_core::{DbColumn, DbObject, ObjectKind, Relation};
use sqlx::{Row, SqlitePool};

/// `sqlite_master.type` → kind; indexes are not listed.
fn kind(ty: &str) -> Option<ObjectKind> {
    match ty {
        "table" => Some(ObjectKind::Table),
        "view" => Some(ObjectKind::View),
        "trigger" => Some(ObjectKind::Trigger),
        _ => None,
    }
}

/// Every table, view and trigger, grouped by type then name.
pub async fn list_objects(pool: &SqlitePool) -> Result<Vec<DbObject>> {
    let rows = sqlx::query(
        "SELECT name, type FROM sqlite_master \
         WHERE type IN ('table', 'view', 'trigger') \
         ORDER BY type, name",
    )
    .fetch_all(pool)
    .await?;
    let mut out = Vec::with_capacity(rows.len());
    for row in rows {
        let ty: String = row.try_get("type")?;
        if let Some(kind) = kind(&ty) {
            out.push(DbObject::new(None, row.try_get::<String, _>("name")?, kind));
        }
    }
    Ok(out)
}

/// User tables and views with their columns, in one round trip
/// (SQLite's internal `sqlite_*` tables are left out).
pub async fn list_relations(pool: &SqlitePool) -> Result<Vec<Relation>> {
    let rows = sqlx::query(
        "SELECT m.name AS name, m.type AS type, p.name AS col, p.type AS typ \
         FROM sqlite_master m JOIN pragma_table_info(m.name) p \
         WHERE m.type IN ('table', 'view') AND m.name NOT LIKE 'sqlite_%' \
         ORDER BY m.name, p.cid",
    )
    .fetch_all(pool)
    .await?;
    let mut out: Vec<Relation> = Vec::new();
    for row in rows {
        let ty: String = row.try_get("type")?;
        let Some(kind) = kind(&ty) else { continue };
        let object = DbObject::new(None, row.try_get::<String, _>("name")?, kind);
        let column = DbColumn {
            name: row.try_get("col")?,
            data_type: row.try_get("typ")?,
        };
        match out.last_mut() {
            Some(last) if last.object == object => last.columns.push(column),
            _ => out.push(Relation {
                object,
                columns: vec![column],
            }),
        }
    }
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;
    use sqlx::sqlite::SqlitePoolOptions;

    #[tokio::test]
    async fn lists_objects_and_relations() {
        let pool = SqlitePoolOptions::new()
            .max_connections(1)
            .connect("sqlite::memory:")
            .await
            .unwrap();
        for sql in [
            "CREATE TABLE t (id INTEGER PRIMARY KEY AUTOINCREMENT, name TEXT)",
            "CREATE VIEW v AS SELECT name FROM t",
            "CREATE TRIGGER tr AFTER INSERT ON t BEGIN SELECT 1; END",
            "CREATE INDEX t_name ON t (name)",
        ] {
            sqlx::query(sql).execute(&pool).await.unwrap();
        }
        let objects: Vec<_> = list_objects(&pool)
            .await
            .unwrap()
            .into_iter()
            .map(|o| (o.name, o.kind))
            .collect();
        assert_eq!(
            objects,
            [
                ("sqlite_sequence".to_string(), ObjectKind::Table),
                ("t".to_string(), ObjectKind::Table),
                ("tr".to_string(), ObjectKind::Trigger),
                ("v".to_string(), ObjectKind::View),
            ]
        );
        let relations: Vec<_> = list_relations(&pool)
            .await
            .unwrap()
            .into_iter()
            .map(|r| {
                let cols: Vec<_> = r.columns.into_iter().map(|c| c.name).collect();
                (r.object.name, cols)
            })
            .collect();
        assert_eq!(
            relations,
            [
                ("t".to_string(), vec!["id".to_string(), "name".to_string()]),
                ("v".to_string(), vec!["name".to_string()]),
            ]
        );
    }
}
