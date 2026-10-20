// SQLite table inspector source: columns, indexes, constraints, DDL.

use futures::future::BoxFuture;
use gpui_kit::SharedString;
use sqlx::{AssertSqlSafe, Row, SqlitePool};

use super::constraints::load_table_constraints;
use crate::widgets::virtual_table::data_column;
use crate::workspace::panels::table_inspector::{InspectSource, Kind, Section, TabDef};

pub struct SqliteInspect {
    pub pool: SqlitePool,
    pub table: String,
}

fn yes_no(b: bool) -> SharedString {
    if b { "YES" } else { "NO" }.into()
}

impl InspectSource for SqliteInspect {
    fn panel_name(&self) -> &'static str {
        "SqliteTableInspector"
    }

    fn breadcrumb_id(&self) -> &'static str {
        "sqlite-insp-breadcrumb"
    }

    fn path(&self) -> Vec<String> {
        vec![self.table.clone()]
    }

    fn tabs(&self) -> Vec<TabDef> {
        let tab = |id, label, kind| TabDef { id, label, kind };
        vec![
            tab("sql-insp-col", "Columns", Kind::Table),
            tab("sql-insp-ix", "Indexes", Kind::Table),
            tab("sql-insp-con", "Constraints", Kind::Table),
            tab("sql-insp-ddl", "DDL", Kind::Code),
        ]
    }

    fn load(&self) -> BoxFuture<'static, anyhow::Result<Vec<Section>>> {
        let (pool, table) = (self.pool.clone(), self.table.clone());
        Box::pin(async move {
            let col_sql = format!("PRAGMA table_info(\"{table}\")");
            let col_rows = sqlx::query(AssertSqlSafe(col_sql)).fetch_all(&pool).await?;
            let columns = Section::Table {
                columns: vec![
                    data_column("cid", "cid"),
                    data_column("name", "Name"),
                    data_column("type", "Type"),
                    data_column("notnull", "NOT NULL"),
                    data_column("pk", "PK"),
                ],
                rows: col_rows
                    .iter()
                    .map(|row| {
                        vec![
                            row.try_get::<i64, _>("cid").unwrap_or(0).to_string().into(),
                            row.try_get::<String, _>("name").unwrap_or_default().into(),
                            row.try_get::<String, _>("type").unwrap_or_default().into(),
                            yes_no(row.try_get::<bool, _>("notnull").unwrap_or(false)),
                            row.try_get::<i64, _>("pk").unwrap_or(0).to_string().into(),
                        ]
                    })
                    .collect(),
            };

            let idx_sql = format!("PRAGMA index_list(\"{table}\")");
            let idx_rows = sqlx::query(AssertSqlSafe(idx_sql)).fetch_all(&pool).await?;
            let indexes = Section::Table {
                columns: vec![data_column("name", "Name"), data_column("unique", "Unique")],
                rows: idx_rows
                    .iter()
                    .map(|row| {
                        vec![
                            row.try_get::<String, _>("name").unwrap_or_default().into(),
                            yes_no(row.try_get::<bool, _>("unique").unwrap_or(false)),
                        ]
                    })
                    .collect(),
            };

            let constraints = Section::Table {
                columns: vec![
                    data_column("name", "Constraint"),
                    data_column("typ", "Type"),
                    data_column("def", "Definition"),
                ],
                rows: load_table_constraints(&pool, &table)
                    .await?
                    .into_iter()
                    .map(|c| vec![c.name.into(), c.constraint_type.into(), c.definition.into()])
                    .collect(),
            };

            let ddl: String = sqlx::query_scalar::<_, String>(
                "SELECT COALESCE(sql, '') FROM sqlite_master WHERE type IN ('table','view') AND name = ?",
            )
            .bind(&table)
            .fetch_optional(&pool)
            .await?
            .unwrap_or_default();
            let ddl = if ddl.is_empty() {
                "(no DDL in sqlite_master)".to_string()
            } else {
                ddl
            };

            Ok(vec![columns, indexes, constraints, Section::Code(ddl)])
        })
    }
}
