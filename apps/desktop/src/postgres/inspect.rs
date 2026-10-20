// Postgres table inspector source: columns, indexes, constraints, stats.

use futures::future::BoxFuture;
use gpui_kit::SharedString;
use sqlx::{PgPool, Row, postgres::PgRow};

use crate::widgets::virtual_table::data_column;
use crate::workspace::panels::table_inspector::{InspectSource, Kind, Section, TabDef};

pub struct PgInspect {
    pub pool: PgPool,
    pub schema: String,
    pub table: String,
}

fn text(row: &PgRow, col: &str) -> SharedString {
    row.try_get::<String, _>(col).unwrap_or_default().into()
}

fn opt_text(row: &PgRow, col: &str) -> String {
    row.try_get::<Option<String>, _>(col)
        .ok()
        .flatten()
        .unwrap_or_default()
}

impl InspectSource for PgInspect {
    fn panel_name(&self) -> &'static str {
        "PgTableInspector"
    }

    fn breadcrumb_id(&self) -> &'static str {
        "pg-insp-breadcrumb"
    }

    fn path(&self) -> Vec<String> {
        vec![self.schema.clone(), self.table.clone()]
    }

    fn tabs(&self) -> Vec<TabDef> {
        let tab = |id, label, kind| TabDef { id, label, kind };
        vec![
            tab("pg-insp-columns", "Columns", Kind::Table),
            tab("pg-insp-indexes", "Indexes", Kind::Table),
            tab("pg-insp-constraints", "Constraints", Kind::Table),
            tab("pg-insp-stats", "Stats", Kind::Facts),
        ]
    }

    fn load(&self) -> BoxFuture<'static, anyhow::Result<Vec<Section>>> {
        let (pool, schema, table) = (self.pool.clone(), self.schema.clone(), self.table.clone());
        Box::pin(async move {
            let col_rows = sqlx::query(
                r"SELECT ordinal_position, column_name, data_type, is_nullable, column_default
                   FROM information_schema.columns
                   WHERE table_schema = $1 AND table_name = $2
                   ORDER BY ordinal_position",
            )
            .bind(&schema)
            .bind(&table)
            .fetch_all(&pool)
            .await
            .unwrap_or_default();

            let idx_rows = sqlx::query(
                r"SELECT indexname, indexdef FROM pg_indexes
                   WHERE schemaname = $1 AND tablename = $2
                   ORDER BY indexname",
            )
            .bind(&schema)
            .bind(&table)
            .fetch_all(&pool)
            .await
            .unwrap_or_default();

            let con_rows = sqlx::query(
                r"SELECT con.conname AS constraint_name,
                       CASE con.contype
                         WHEN 'p' THEN 'PRIMARY KEY'
                         WHEN 'u' THEN 'UNIQUE'
                         WHEN 'f' THEN 'FOREIGN KEY'
                         WHEN 'c' THEN 'CHECK'
                         ELSE con.contype::text
                       END AS constraint_type,
                       pg_get_constraintdef(con.oid) AS definition
                FROM pg_constraint con
                JOIN pg_class rel ON rel.oid = con.conrelid
                JOIN pg_namespace nsp ON nsp.oid = rel.relnamespace
                WHERE nsp.nspname = $1 AND rel.relname = $2
                ORDER BY con.conname",
            )
            .bind(&schema)
            .bind(&table)
            .fetch_all(&pool)
            .await
            .unwrap_or_default();

            let stat_rows = sqlx::query(
                r"SELECT
                       c.reltuples::bigint AS estimate_rows,
                       pg_size_pretty(pg_relation_size(c.oid)) AS table_size,
                       pg_size_pretty(pg_indexes_size(c.oid)) AS indexes_size,
                       pg_size_pretty(pg_total_relation_size(c.oid)) AS total_size
                FROM pg_class c
                JOIN pg_namespace n ON n.oid = c.relnamespace
                WHERE n.nspname = $1 AND c.relname = $2 AND c.relkind IN ('r', 'p', 'm')",
            )
            .bind(&schema)
            .bind(&table)
            .fetch_optional(&pool)
            .await
            .unwrap_or(None);

            let columns = Section::Table {
                columns: vec![
                    data_column("pos", "#"),
                    data_column("name", "Column"),
                    data_column("type", "Type"),
                    data_column("nullable", "NULL"),
                    data_column("default", "Default"),
                ],
                rows: col_rows
                    .iter()
                    .map(|row| {
                        let pos: i32 = row.try_get("ordinal_position").unwrap_or(0);
                        vec![
                            pos.to_string().into(),
                            text(row, "column_name"),
                            text(row, "data_type"),
                            text(row, "is_nullable"),
                            opt_text(row, "column_default").into(),
                        ]
                    })
                    .collect(),
            };

            let indexes = Section::Table {
                columns: vec![
                    data_column("name", "Index"),
                    data_column("def", "Definition"),
                ],
                rows: idx_rows
                    .iter()
                    .map(|row| vec![text(row, "indexname"), text(row, "indexdef")])
                    .collect(),
            };

            let constraints = Section::Table {
                columns: vec![
                    data_column("name", "Constraint"),
                    data_column("typ", "Type"),
                    data_column("def", "Definition"),
                ],
                rows: con_rows
                    .iter()
                    .map(|row| {
                        vec![
                            text(row, "constraint_name"),
                            text(row, "constraint_type"),
                            text(row, "definition"),
                        ]
                    })
                    .collect(),
            };

            let stats = match stat_rows.as_ref() {
                Some(sr) => {
                    let estimate = sr
                        .try_get::<i64, _>("estimate_rows")
                        .map_or_else(|_| "—".to_string(), |n| n.to_string());
                    vec![
                        ("Estimated rows".into(), estimate.into()),
                        ("Table size".into(), opt_text(sr, "table_size").into()),
                        ("Indexes size".into(), opt_text(sr, "indexes_size").into()),
                        ("Total size".into(), opt_text(sr, "total_size").into()),
                    ]
                }
                None => vec![(
                    "(no stats)".into(),
                    "relation not found or not a regular table".into(),
                )],
            };

            Ok(vec![columns, indexes, constraints, Section::Facts(stats)])
        })
    }
}
