//! Per-column catalog metadata (types, keys) for table browse.

use std::collections::HashSet;

use anyhow::Result;
use based_core::{ColumnCatalog, ColumnMeta, FkTarget, ObjectRef, quote_ident};
use sqlx::{AssertSqlSafe, Row, SqlitePool};

pub async fn column_catalog(pool: &SqlitePool, table: &str) -> Result<ColumnCatalog> {
    let q = quote_ident(table);
    let col_sql = format!("PRAGMA table_info({q})");
    let col_rows = sqlx::query(AssertSqlSafe(col_sql)).fetch_all(pool).await?;

    let mut out = ColumnCatalog::default();
    let mut pk_columns = HashSet::new();
    for row in &col_rows {
        let name: String = row.try_get("name")?;
        let type_name: String = row.try_get("type")?;
        let notnull: bool = row.try_get("notnull")?;
        let pk: i64 = row.try_get("pk")?;
        if pk > 0 {
            pk_columns.insert(name.clone());
        }
        out.push(
            name.clone(),
            ColumnMeta {
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

    let fk_sql = format!("PRAGMA foreign_key_list({q})");
    let fk_rows = sqlx::query(AssertSqlSafe(fk_sql)).fetch_all(pool).await?;
    for row in fk_rows {
        let from_col: String = row.try_get("from")?;
        let to_table: String = row.try_get("table")?;
        // NULL for `REFERENCES parent` without a column list: the parent's
        // primary key, matched by position (`seq`) for composite keys.
        let to_col = match row.try_get::<Option<String>, _>("to")? {
            Some(col) => Some(col),
            None => {
                let seq: i64 = row.try_get("seq")?;
                primary_key_columns(pool, &to_table)
                    .await?
                    .into_iter()
                    .nth(usize::try_from(seq).unwrap_or(usize::MAX))
            }
        };
        if let (Some(meta), Some(column)) = (out.get_mut(&from_col), to_col)
            && meta.fk.is_none()
        {
            meta.fk = Some(FkTarget {
                table: ObjectRef::bare(to_table),
                column,
            });
        }
    }

    let idx_sql = format!("PRAGMA index_list({q})");
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
        let info_sql = format!("PRAGMA index_info({})", quote_ident(&idx_name));
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

/// A table's primary-key columns in key order (empty for rowid-only tables).
async fn primary_key_columns(pool: &SqlitePool, table: &str) -> Result<Vec<String>> {
    let sql = format!("PRAGMA table_info({})", quote_ident(table));
    let mut cols: Vec<(i64, String)> = Vec::new();
    for row in sqlx::query(AssertSqlSafe(sql)).fetch_all(pool).await? {
        let pk: i64 = row.try_get("pk")?;
        if pk > 0 {
            cols.push((pk, row.try_get("name")?));
        }
    }
    cols.sort();
    Ok(cols.into_iter().map(|(_, name)| name).collect())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn reads_keys_and_types() {
        let pool = SqlitePool::connect("sqlite::memory:").await.unwrap();
        for sql in [
            "CREATE TABLE p (id INTEGER PRIMARY KEY)",
            "CREATE TABLE \"c t\" (id INTEGER PRIMARY KEY, pid INTEGER REFERENCES p(id), email TEXT NOT NULL)",
            "CREATE UNIQUE INDEX ux ON \"c t\"(email)",
        ] {
            sqlx::query(sql).execute(&pool).await.unwrap();
        }
        let m = column_catalog(&pool, "c t").await.unwrap();
        assert_eq!(m.names().collect::<Vec<_>>(), ["id", "pid", "email"]);
        assert!(m["id"].is_primary_key);
        assert_eq!(
            m["pid"].fk,
            Some(FkTarget {
                table: ObjectRef::bare("p"),
                column: "id".into()
            })
        );
        assert!(m["email"].is_unique && m["email"].nullable == Some(false));
    }

    #[tokio::test]
    async fn implied_fk_resolves_the_parent_primary_key() {
        let pool = SqlitePool::connect("sqlite::memory:").await.unwrap();
        for sql in [
            "CREATE TABLE users (id INTEGER PRIMARY KEY)",
            "CREATE TABLE orders (id INTEGER PRIMARY KEY, user_id INTEGER REFERENCES users)",
            "CREATE TABLE pair (a INT, b INT, PRIMARY KEY (a, b))",
            "CREATE TABLE child (x INT, y INT, FOREIGN KEY (x, y) REFERENCES pair)",
        ] {
            sqlx::query(sql).execute(&pool).await.unwrap();
        }
        let m = column_catalog(&pool, "orders").await.unwrap();
        let fk = m["user_id"].fk.clone().unwrap();
        assert_eq!(
            (fk.table.name.as_str(), fk.column.as_str()),
            ("users", "id")
        );
        let m = column_catalog(&pool, "child").await.unwrap();
        assert_eq!(m["x"].fk.as_ref().unwrap().column, "a");
        assert_eq!(m["y"].fk.as_ref().unwrap().column, "b");
    }
}
