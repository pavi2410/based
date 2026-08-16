//! SQLite table constraints assembled from PRAGMA table_info / index_list / foreign_key_list.

use std::collections::BTreeMap;

use anyhow::Result;
use sqlx::{AssertSqlSafe, Row, SqlitePool};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ConstraintInfo {
    pub name: String,
    pub constraint_type: String,
    pub definition: String,
}

pub async fn load_table_constraints(
    pool: &SqlitePool,
    table_name: &str,
) -> Result<Vec<ConstraintInfo>> {
    let mut out = Vec::new();
    push_primary_key(pool, table_name, &mut out).await?;
    push_unique_constraints(pool, table_name, &mut out).await?;
    push_foreign_keys(pool, table_name, &mut out).await?;
    Ok(out)
}

async fn push_primary_key(
    pool: &SqlitePool,
    table_name: &str,
    out: &mut Vec<ConstraintInfo>,
) -> Result<()> {
    let col_sql = format!("PRAGMA table_info(\"{table_name}\")");
    let col_rows = sqlx::query(AssertSqlSafe(col_sql)).fetch_all(pool).await?;
    let mut pk_cols: Vec<(i64, String)> = col_rows
        .iter()
        .filter_map(|row| {
            let pk: i64 = row.try_get("pk").ok()?;
            if pk <= 0 {
                return None;
            }
            let name: String = row.try_get("name").ok()?;
            Some((pk, name))
        })
        .collect();
    if pk_cols.is_empty() {
        return Ok(());
    }
    pk_cols.sort_by_key(|(pk, _)| *pk);

    let name = pk_index_name(pool, table_name).await?.unwrap_or_default();
    let cols = pk_cols
        .into_iter()
        .map(|(_, name)| name)
        .collect::<Vec<_>>()
        .join(", ");
    out.push(ConstraintInfo {
        name,
        constraint_type: "PRIMARY KEY".into(),
        definition: format!("PRIMARY KEY ({cols})"),
    });
    Ok(())
}

async fn pk_index_name(pool: &SqlitePool, table_name: &str) -> Result<Option<String>> {
    let idx_sql = format!("PRAGMA index_list(\"{table_name}\")");
    let idx_rows = sqlx::query(AssertSqlSafe(idx_sql)).fetch_all(pool).await?;
    for row in idx_rows {
        let origin: String = row.try_get("origin").unwrap_or_default();
        if origin == "pk" {
            return Ok(Some(row.try_get("name").unwrap_or_default()));
        }
    }
    Ok(None)
}

async fn push_unique_constraints(
    pool: &SqlitePool,
    table_name: &str,
    out: &mut Vec<ConstraintInfo>,
) -> Result<()> {
    let idx_sql = format!("PRAGMA index_list(\"{table_name}\")");
    let idx_rows = sqlx::query(AssertSqlSafe(idx_sql)).fetch_all(pool).await?;
    for idx in idx_rows {
        let origin: String = idx.try_get("origin").unwrap_or_default();
        if origin != "u" {
            continue;
        }
        let name: String = idx.try_get("name").unwrap_or_default();
        let cols = index_columns(pool, &name).await?;
        out.push(ConstraintInfo {
            name,
            constraint_type: "UNIQUE".into(),
            definition: format!("UNIQUE ({})", cols.join(", ")),
        });
    }
    Ok(())
}

async fn index_columns(pool: &SqlitePool, index_name: &str) -> Result<Vec<String>> {
    let info_sql = format!("PRAGMA index_info(\"{index_name}\")");
    let info_rows = sqlx::query(AssertSqlSafe(info_sql)).fetch_all(pool).await?;
    let mut cols: Vec<(i64, String)> = info_rows
        .iter()
        .map(|row| {
            (
                row.try_get("seqno").unwrap_or(0),
                row.try_get("name").unwrap_or_default(),
            )
        })
        .collect();
    cols.sort_by_key(|(seq, _)| *seq);
    Ok(cols.into_iter().map(|(_, name)| name).collect())
}

async fn push_foreign_keys(
    pool: &SqlitePool,
    table_name: &str,
    out: &mut Vec<ConstraintInfo>,
) -> Result<()> {
    let fk_sql = format!("PRAGMA foreign_key_list(\"{table_name}\")");
    let fk_rows = sqlx::query(AssertSqlSafe(fk_sql)).fetch_all(pool).await?;

    struct FkGroup {
        table: String,
        on_update: String,
        on_delete: String,
        cols: Vec<(i64, String, Option<String>)>,
    }

    let mut groups: BTreeMap<i64, FkGroup> = BTreeMap::new();
    for row in fk_rows {
        let id: i64 = row.try_get("id").unwrap_or(0);
        let seq: i64 = row.try_get("seq").unwrap_or(0);
        let table: String = row.try_get("table").unwrap_or_default();
        let from: String = row.try_get("from").unwrap_or_default();
        let to: Option<String> = row.try_get("to").ok().flatten();
        let on_update: String = row.try_get("on_update").unwrap_or_default();
        let on_delete: String = row.try_get("on_delete").unwrap_or_default();
        groups
            .entry(id)
            .or_insert(FkGroup {
                table,
                on_update,
                on_delete,
                cols: Vec::new(),
            })
            .cols
            .push((seq, from, to));
    }

    for group in groups.into_values() {
        let mut cols = group.cols;
        cols.sort_by_key(|(seq, _, _)| *seq);
        let froms: Vec<&str> = cols.iter().map(|(_, from, _)| from.as_str()).collect();
        let tos: Vec<&str> = cols.iter().filter_map(|(_, _, to)| to.as_deref()).collect();
        let mut definition = format!(
            "FOREIGN KEY ({}) REFERENCES {}",
            froms.join(", "),
            group.table
        );
        if tos.len() == froms.len() {
            definition.push_str(&format!("({})", tos.join(", ")));
        }
        append_fk_action(&mut definition, "ON DELETE", &group.on_delete);
        append_fk_action(&mut definition, "ON UPDATE", &group.on_update);
        out.push(ConstraintInfo {
            name: String::new(),
            constraint_type: "FOREIGN KEY".into(),
            definition,
        });
    }
    Ok(())
}

fn append_fk_action(definition: &mut String, clause: &str, action: &str) {
    if action.is_empty() || action.eq_ignore_ascii_case("NO ACTION") {
        return;
    }
    definition.push(' ');
    definition.push_str(clause);
    definition.push(' ');
    definition.push_str(action);
}

#[cfg(test)]
mod tests {
    use super::*;

    async fn mem_pool() -> SqlitePool {
        SqlitePool::connect("sqlite::memory:").await.unwrap()
    }

    async fn exec(pool: &SqlitePool, sql: &'static str) {
        sqlx::query(sql).execute(pool).await.unwrap();
    }

    fn of_type<'a>(rows: &'a [ConstraintInfo], typ: &str) -> Vec<&'a ConstraintInfo> {
        rows.iter().filter(|c| c.constraint_type == typ).collect()
    }

    #[tokio::test]
    async fn empty_table_has_no_constraints() {
        let pool = mem_pool().await;
        exec(&pool, "CREATE TABLE t (name TEXT)").await;
        let rows = load_table_constraints(&pool, "t").await.unwrap();
        assert!(rows.is_empty());
    }

    #[tokio::test]
    async fn integer_primary_key() {
        let pool = mem_pool().await;
        exec(&pool, "CREATE TABLE t (id INTEGER PRIMARY KEY, name TEXT)").await;
        let rows = load_table_constraints(&pool, "t").await.unwrap();
        let pks = of_type(&rows, "PRIMARY KEY");
        assert_eq!(pks.len(), 1);
        assert_eq!(pks[0].definition, "PRIMARY KEY (id)");
    }

    #[tokio::test]
    async fn composite_primary_key_preserves_order() {
        let pool = mem_pool().await;
        exec(
            &pool,
            "CREATE TABLE t (a INT, b INT, c INT, PRIMARY KEY (c, a))",
        )
        .await;
        let rows = load_table_constraints(&pool, "t").await.unwrap();
        let pks = of_type(&rows, "PRIMARY KEY");
        assert_eq!(pks.len(), 1);
        assert_eq!(pks[0].definition, "PRIMARY KEY (c, a)");
        assert!(!pks[0].name.is_empty());
    }

    #[tokio::test]
    async fn unique_column_constraint() {
        let pool = mem_pool().await;
        exec(
            &pool,
            "CREATE TABLE t (id INTEGER PRIMARY KEY, email TEXT UNIQUE)",
        )
        .await;
        let rows = load_table_constraints(&pool, "t").await.unwrap();
        let uqs = of_type(&rows, "UNIQUE");
        assert_eq!(uqs.len(), 1);
        assert_eq!(uqs[0].definition, "UNIQUE (email)");
        assert!(!uqs[0].name.is_empty());
    }

    #[tokio::test]
    async fn create_unique_index_is_not_a_constraint() {
        let pool = mem_pool().await;
        exec(&pool, "CREATE TABLE t (id INTEGER PRIMARY KEY, email TEXT)").await;
        exec(&pool, "CREATE UNIQUE INDEX idx_email ON t (email)").await;
        let rows = load_table_constraints(&pool, "t").await.unwrap();
        assert!(of_type(&rows, "UNIQUE").is_empty());
    }

    #[tokio::test]
    async fn foreign_key_with_on_delete() {
        let pool = mem_pool().await;
        exec(&pool, "CREATE TABLE users (id INTEGER PRIMARY KEY)").await;
        exec(
            &pool,
            "CREATE TABLE orders (id INTEGER PRIMARY KEY, user_id INTEGER, \
             FOREIGN KEY (user_id) REFERENCES users(id) ON DELETE CASCADE)",
        )
        .await;
        let rows = load_table_constraints(&pool, "orders").await.unwrap();
        let fks = of_type(&rows, "FOREIGN KEY");
        assert_eq!(fks.len(), 1);
        assert_eq!(
            fks[0].definition,
            "FOREIGN KEY (user_id) REFERENCES users(id) ON DELETE CASCADE"
        );
    }

    #[tokio::test]
    async fn composite_foreign_key() {
        let pool = mem_pool().await;
        exec(
            &pool,
            "CREATE TABLE parent (a INT, b INT, PRIMARY KEY (a, b))",
        )
        .await;
        exec(
            &pool,
            "CREATE TABLE child (x INT, y INT, FOREIGN KEY (x, y) REFERENCES parent(a, b))",
        )
        .await;
        let rows = load_table_constraints(&pool, "child").await.unwrap();
        let fks = of_type(&rows, "FOREIGN KEY");
        assert_eq!(fks.len(), 1);
        assert_eq!(
            fks[0].definition,
            "FOREIGN KEY (x, y) REFERENCES parent(a, b)"
        );
    }

    #[tokio::test]
    async fn implied_fk_target_column() {
        let pool = mem_pool().await;
        exec(&pool, "CREATE TABLE users (id INTEGER PRIMARY KEY)").await;
        exec(
            &pool,
            "CREATE TABLE orders (id INTEGER PRIMARY KEY, user_id INTEGER REFERENCES users)",
        )
        .await;
        let rows = load_table_constraints(&pool, "orders").await.unwrap();
        let fks = of_type(&rows, "FOREIGN KEY");
        assert_eq!(fks.len(), 1);
        assert_eq!(fks[0].definition, "FOREIGN KEY (user_id) REFERENCES users");
    }
}
