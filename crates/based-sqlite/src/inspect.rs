//! Table structure for the inspector: columns, indexes, constraints, DDL.

use anyhow::Result;
use based_core::{Grid, TableExtra, TableInfo, quote_ident};
use sqlx::{AssertSqlSafe, Row, SqlitePool};

use crate::constraints::load_table_constraints;

fn yes_no(b: bool) -> String {
    if b { "YES" } else { "NO" }.into()
}

pub async fn inspect_table(pool: &SqlitePool, table: &str) -> Result<TableInfo> {
    let q = quote_ident(table);
    let cols = sqlx::query(AssertSqlSafe(format!("PRAGMA table_info({q})")))
        .fetch_all(pool)
        .await?;
    let columns = Grid::new(
        &["cid", "Name", "Type", "NOT NULL", "PK"],
        cols.iter()
            .map(|r| {
                vec![
                    r.try_get::<i64, _>("cid").unwrap_or(0).to_string(),
                    r.try_get("name").unwrap_or_default(),
                    r.try_get("type").unwrap_or_default(),
                    yes_no(r.try_get("notnull").unwrap_or(false)),
                    r.try_get::<i64, _>("pk").unwrap_or(0).to_string(),
                ]
            })
            .collect(),
    );

    let idx = sqlx::query(AssertSqlSafe(format!("PRAGMA index_list({q})")))
        .fetch_all(pool)
        .await?;
    let indexes = Grid::new(
        &["Name", "Unique"],
        idx.iter()
            .map(|r| {
                vec![
                    r.try_get("name").unwrap_or_default(),
                    yes_no(r.try_get("unique").unwrap_or(false)),
                ]
            })
            .collect(),
    );

    let constraints = Grid::new(
        &["Constraint", "Type", "Definition"],
        load_table_constraints(pool, table)
            .await?
            .into_iter()
            .map(|c| vec![c.name, c.constraint_type, c.definition])
            .collect(),
    );

    let ddl: String = sqlx::query_scalar(
        "SELECT COALESCE(sql, '') FROM sqlite_master WHERE type IN ('table','view') AND name = ?",
    )
    .bind(table)
    .fetch_optional(pool)
    .await?
    .unwrap_or_default();
    let ddl = if ddl.is_empty() {
        "(no DDL in sqlite_master)".into()
    } else {
        ddl
    };

    Ok(TableInfo {
        columns,
        indexes,
        constraints,
        extra: TableExtra::Ddl(ddl),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn inspects_table() {
        let pool = SqlitePool::connect("sqlite::memory:").await.unwrap();
        sqlx::query("CREATE TABLE t (id INTEGER PRIMARY KEY, n TEXT NOT NULL)")
            .execute(&pool)
            .await
            .unwrap();
        let info = inspect_table(&pool, "t").await.unwrap();
        assert_eq!(info.columns.rows.len(), 2);
        assert_eq!(info.columns.rows[1][3], "YES");
        assert!(matches!(info.extra, TableExtra::Ddl(d) if d.starts_with("CREATE TABLE")));
    }
}
