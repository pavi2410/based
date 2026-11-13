//! The objects a connection lists: tables, views and materialized views.
//!
//! Read from `pg_class` rather than `information_schema.tables`, which leaves
//! out materialized views. The visibility filter matches `information_schema`:
//! objects the user owns or holds any privilege on.

use anyhow::Result;
use based_core::{DbColumn, DbObject, ObjectKind, Relation};
use sqlx::{AssertSqlSafe, PgPool, Row};

/// Relation kinds listed, with `relkind` as Postgres stores it.
const LISTED: &str = "'r', 'p', 'f', 'v', 'm'";

const VISIBLE: &str = "n.nspname NOT IN ('pg_catalog', 'information_schema') \
     AND n.nspname NOT LIKE 'pg_toast%' \
     AND n.nspname NOT LIKE 'pg_temp_%' \
     AND (pg_has_role(c.relowner, 'USAGE') \
          OR has_table_privilege(c.oid, 'SELECT, INSERT, UPDATE, DELETE, TRUNCATE, REFERENCES, TRIGGER') \
          OR has_any_column_privilege(c.oid, 'SELECT, INSERT, UPDATE, REFERENCES'))";

/// `relkind` → kind. Ordinary, partitioned and foreign tables are all tables.
fn kind(relkind: &str) -> Option<ObjectKind> {
    match relkind {
        "r" | "p" | "f" => Some(ObjectKind::Table),
        "v" => Some(ObjectKind::View),
        "m" => Some(ObjectKind::MaterializedView),
        _ => None,
    }
}

/// Every visible table, view and materialized view, by schema then name.
pub async fn list_objects(pool: &PgPool) -> Result<Vec<DbObject>> {
    let rows = sqlx::query(AssertSqlSafe(format!(
        "SELECT n.nspname AS sch, c.relname AS name, c.relkind::text AS kind \
         FROM pg_class c JOIN pg_namespace n ON n.oid = c.relnamespace \
         WHERE c.relkind IN ({LISTED}) AND {VISIBLE} \
         ORDER BY n.nspname, c.relname"
    )))
    .fetch_all(pool)
    .await?;
    let mut out = Vec::with_capacity(rows.len());
    for row in rows {
        let relkind: String = row.try_get("kind")?;
        if let Some(kind) = kind(&relkind) {
            out.push(DbObject::new(
                Some(row.try_get("sch")?),
                row.try_get::<String, _>("name")?,
                kind,
            ));
        }
    }
    Ok(out)
}

/// The same objects with their columns, in column order, in one round trip.
pub async fn list_relations(pool: &PgPool) -> Result<Vec<Relation>> {
    let rows = sqlx::query(AssertSqlSafe(format!(
        "SELECT n.nspname AS sch, c.relname AS name, c.relkind::text AS kind, \
                a.attname AS col, format_type(a.atttypid, a.atttypmod) AS typ \
         FROM pg_class c \
         JOIN pg_namespace n ON n.oid = c.relnamespace \
         JOIN pg_attribute a ON a.attrelid = c.oid AND a.attnum > 0 AND NOT a.attisdropped \
         WHERE c.relkind IN ({LISTED}) AND {VISIBLE} \
         ORDER BY n.nspname, c.relname, a.attnum"
    )))
    .fetch_all(pool)
    .await?;
    let mut out: Vec<Relation> = Vec::new();
    for row in rows {
        let relkind: String = row.try_get("kind")?;
        let Some(kind) = kind(&relkind) else { continue };
        let object = DbObject::new(
            Some(row.try_get("sch")?),
            row.try_get::<String, _>("name")?,
            kind,
        );
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
