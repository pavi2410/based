//! A table's structure from SQLite's own pragmas: `table_xinfo`,
//! `foreign_key_list`, `index_list` / `index_info`, and the stored DDL.

use std::collections::BTreeMap;

use anyhow::Result;
use based_core::{
    ColumnRef, ColumnSpec, ConstraintSpec, IndexSpec, ObjectRef, Reference, TableSpec, quote_ident,
};
use sqlx::{AssertSqlSafe, Row, SqlitePool};

use crate::constraints::load_table_constraints;

/// One foreign key of a table, its columns in key order.
struct ForeignKey {
    table: String,
    from: Vec<String>,
    /// Empty `to` names mean "the target's primary key".
    to: Vec<Option<String>>,
}

async fn foreign_keys(pool: &SqlitePool, table: &str) -> Result<Vec<ForeignKey>> {
    let rows = sqlx::query(AssertSqlSafe(format!(
        "PRAGMA foreign_key_list({})",
        quote_ident(table)
    )))
    .fetch_all(pool)
    .await?;
    let mut by_id: BTreeMap<i64, ForeignKey> = BTreeMap::new();
    for r in &rows {
        let fk = by_id.entry(r.try_get("id")?).or_insert_with(|| ForeignKey {
            table: r.try_get("table").unwrap_or_default(),
            from: Vec::new(),
            to: Vec::new(),
        });
        fk.from.push(r.try_get("from")?);
        fk.to.push(r.try_get("to")?);
    }
    Ok(by_id.into_values().collect())
}

async fn primary_key(pool: &SqlitePool, table: &str) -> Result<Vec<String>> {
    let rows = sqlx::query(AssertSqlSafe(format!(
        "PRAGMA table_info({})",
        quote_ident(table)
    )))
    .fetch_all(pool)
    .await?;
    let mut pk: Vec<(i64, String)> = rows
        .iter()
        .filter_map(|r| {
            let n: i64 = r.try_get("pk").ok()?;
            (n > 0).then(|| Some((n, r.try_get("name").ok()?)))?
        })
        .collect();
    pk.sort();
    Ok(pk.into_iter().map(|(_, name)| name).collect())
}

/// A foreign key's target columns, with "the primary key" spelled out.
async fn target_columns(pool: &SqlitePool, fk: &ForeignKey) -> Result<Vec<String>> {
    if fk.to.iter().all(Option::is_some) {
        return Ok(fk.to.iter().flatten().cloned().collect());
    }
    primary_key(pool, &fk.table).await
}

pub async fn table_spec(pool: &SqlitePool, table: &str) -> Result<TableSpec> {
    let q = quote_ident(table);
    let pk = primary_key(pool, table).await?;

    let mut single_fks: Vec<(String, ColumnRef)> = Vec::new();
    for fk in foreign_keys(pool, table).await? {
        if fk.from.len() == 1 {
            let to = target_columns(pool, &fk).await?;
            if let Some(column) = to.first() {
                single_fks.push((
                    fk.from[0].clone(),
                    ColumnRef {
                        table: ObjectRef::bare(fk.table.clone()),
                        column: column.clone(),
                    },
                ));
            }
        }
    }

    // table_xinfo also lists generated columns (hidden 2 and 3); 1 is a virtual-table hidden column.
    let column_rows = sqlx::query(AssertSqlSafe(format!("PRAGMA table_xinfo({q})")))
        .fetch_all(pool)
        .await?;
    let mut columns = Vec::with_capacity(column_rows.len());
    for r in &column_rows {
        if r.try_get::<i64, _>("hidden").unwrap_or(0) == 1 {
            continue;
        }
        let name: String = r.try_get("name")?;
        let data_type: String = r.try_get("type").unwrap_or_default();
        let primary = pk.contains(&name);
        // An INTEGER PRIMARY KEY is the rowid: never NULL, whatever notnull says.
        let rowid = primary && pk.len() == 1 && data_type.eq_ignore_ascii_case("integer");
        columns.push(ColumnSpec {
            nullable: !r.try_get::<bool, _>("notnull").unwrap_or(false) && !rowid,
            default: r.try_get("dflt_value")?,
            primary_key: primary,
            references: single_fks
                .iter()
                .find(|(c, _)| *c == name)
                .map(|(_, t)| t.clone()),
            name,
            data_type,
        });
    }

    let index_rows = sqlx::query(AssertSqlSafe(format!("PRAGMA index_list({q})")))
        .fetch_all(pool)
        .await?;
    let mut indexes = Vec::with_capacity(index_rows.len());
    for r in &index_rows {
        let name: String = r.try_get("name")?;
        let info = sqlx::query(AssertSqlSafe(format!(
            "PRAGMA index_info({})",
            quote_ident(&name)
        )))
        .fetch_all(pool)
        .await?;
        let columns = info
            .iter()
            .map(|i| {
                i.try_get::<Option<String>, _>("name")
                    .ok()
                    .flatten()
                    .unwrap_or_else(|| "(expression)".into())
            })
            .collect();
        indexes.push(IndexSpec {
            name,
            unique: r.try_get("unique").unwrap_or(false),
            method: None,
            columns,
        });
    }
    // The rowid is an index of its own that index_list doesn't show.
    if let [only] = pk.as_slice()
        && columns
            .iter()
            .any(|c| c.name == *only && c.data_type.eq_ignore_ascii_case("integer"))
    {
        indexes.insert(
            0,
            IndexSpec {
                name: "(rowid)".into(),
                unique: true,
                method: None,
                columns: vec![only.clone()],
            },
        );
    }

    let constraints = load_table_constraints(pool, table)
        .await?
        .into_iter()
        .map(|c| ConstraintSpec {
            name: c.name,
            kind: c.constraint_type,
            definition: c.definition,
        })
        .collect();

    let mut referenced_by = Vec::new();
    let others: Vec<String> = sqlx::query_scalar(
        "SELECT name FROM sqlite_master WHERE type = 'table' AND name NOT LIKE 'sqlite_%' ORDER BY name",
    )
    .fetch_all(pool)
    .await?;
    for other in others {
        for fk in foreign_keys(pool, &other).await? {
            if fk.table.eq_ignore_ascii_case(table) {
                referenced_by.push(Reference {
                    from_table: ObjectRef::bare(other.clone()),
                    to_columns: target_columns(pool, &fk).await?,
                    from_columns: fk.from,
                });
            }
        }
    }

    let ddl: Vec<String> = sqlx::query_scalar(
        "SELECT sql || ';' FROM sqlite_master
         WHERE tbl_name = ? AND sql IS NOT NULL AND type IN ('table', 'view', 'index', 'trigger')
         ORDER BY CASE type WHEN 'table' THEN 0 WHEN 'view' THEN 0 WHEN 'index' THEN 1 ELSE 2 END, name",
    )
    .bind(table)
    .fetch_all(pool)
    .await?;

    Ok(TableSpec {
        columns,
        indexes,
        constraints,
        referenced_by,
        row_estimate: row_estimate(pool, table).await?,
        sizes: None,
        ddl: ddl.join("\n"),
    })
}

/// Rows per `sqlite_stat1`, written by ANALYZE; `None` when it was never run.
async fn row_estimate(pool: &SqlitePool, table: &str) -> Result<Option<i64>> {
    let has_stats: bool = sqlx::query_scalar(
        "SELECT EXISTS (SELECT 1 FROM sqlite_master WHERE type = 'table' AND name = 'sqlite_stat1')",
    )
    .fetch_one(pool)
    .await?;
    if !has_stats {
        return Ok(None);
    }
    let stat: Option<String> =
        sqlx::query_scalar("SELECT stat FROM sqlite_stat1 WHERE tbl = ? LIMIT 1")
            .bind(table)
            .fetch_optional(pool)
            .await?;
    Ok(stat.and_then(|s| s.split_whitespace().next()?.parse().ok()))
}

#[cfg(test)]
mod tests {
    use super::*;

    async fn db(statements: &[&str]) -> SqlitePool {
        let pool = SqlitePool::connect("sqlite::memory:").await.unwrap();
        for sql in statements {
            sqlx::query(AssertSqlSafe(*sql))
                .execute(&pool)
                .await
                .unwrap();
        }
        pool
    }

    #[tokio::test]
    async fn reads_columns_keys_indexes_references_and_ddl() {
        let pool = db(&[
            "CREATE TABLE customers (id INTEGER PRIMARY KEY, email TEXT UNIQUE)",
            "CREATE TABLE orders (
                id INTEGER PRIMARY KEY,
                customer_id INTEGER REFERENCES customers,
                total NUMERIC NOT NULL DEFAULT 0,
                placed_at TEXT
             )",
            "CREATE INDEX orders_by_date ON orders (placed_at, customer_id)",
            "CREATE TABLE items (order_id INTEGER REFERENCES orders (id), sku TEXT)",
        ])
        .await;
        let spec = table_spec(&pool, "orders").await.unwrap();

        let id = &spec.columns[0];
        assert!(
            id.primary_key && !id.nullable,
            "INTEGER PRIMARY KEY is the rowid"
        );
        let customer = &spec.columns[1];
        let target = customer
            .references
            .as_ref()
            .expect("customer_id is a foreign key");
        assert_eq!(target.table, ObjectRef::bare("customers"));
        assert_eq!(
            target.column, "id",
            "a bare REFERENCES means the primary key"
        );
        assert!(spec.is_unindexed_foreign_key(customer));
        assert_eq!(spec.columns[2].default.as_deref(), Some("0"));
        assert!(!spec.columns[2].nullable);

        assert_eq!(spec.indexes_on("id"), ["(rowid)"]);
        assert_eq!(spec.indexes_on("customer_id"), ["orders_by_date"]);

        assert_eq!(spec.referenced_by.len(), 1);
        assert_eq!(spec.referenced_by[0].from_table, ObjectRef::bare("items"));
        assert_eq!(spec.referenced_by[0].from_columns, ["order_id"]);

        assert!(spec.ddl.starts_with("CREATE TABLE orders ("));
        assert!(spec.ddl.contains("CREATE INDEX orders_by_date"));
        assert_eq!(spec.row_estimate, None, "never analyzed");
    }

    #[tokio::test]
    async fn analyze_gives_a_row_estimate() {
        let pool = db(&[
            "CREATE TABLE t (id INTEGER PRIMARY KEY, n TEXT)",
            "CREATE INDEX t_n ON t (n)",
            "INSERT INTO t (n) VALUES ('a'), ('b'), ('c')",
            "ANALYZE",
        ])
        .await;
        let spec = table_spec(&pool, "t").await.unwrap();
        assert_eq!(spec.row_estimate, Some(3));
    }
}
