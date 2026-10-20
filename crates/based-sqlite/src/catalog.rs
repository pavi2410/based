//! Per-column catalog metadata (types, keys) for table browse.

use std::collections::{HashMap, HashSet};

use anyhow::Result;
use based_core::{ColumnMeta, quote_ident};
use sqlx::{AssertSqlSafe, Row, SqlitePool};

pub async fn column_catalog(pool: &SqlitePool, table: &str) -> Result<HashMap<String, ColumnMeta>> {
    let q = quote_ident(table);
    let col_sql = format!("PRAGMA table_info({q})");
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
        let to_col: String = row.try_get("to")?;
        if let Some(meta) = out.get_mut(&from_col) {
            meta.is_foreign_key = true;
            meta.fk_target = Some(format!("{to_table}({to_col})"));
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
        assert!(m["id"].is_primary_key);
        assert_eq!(m["pid"].fk_target.as_deref(), Some("p(id)"));
        assert!(m["email"].is_unique && m["email"].nullable == Some(false));
    }
}
