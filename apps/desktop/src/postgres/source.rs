// Postgres table source for the shared data viewer (schema-qualified).

use std::collections::HashMap;

use gpui_kit::SharedString;
use sqlx::{AssertSqlSafe, Column, PgPool, Row};

use crate::db::column_catalog;
use crate::widgets::column_header::GridColumnMeta;
use crate::widgets::filter_bar::FilterExpr;
use crate::widgets::row_cell::pg_cell_display;
use crate::workspace::panels::data_viewer::{DataSource, Fut, PageData, quote_ident};

pub struct PgSource {
    pub pool: PgPool,
    pub schema: String,
    pub table: String,
}

impl DataSource for PgSource {
    fn panel_name(&self) -> &'static str {
        "PgDataViewer"
    }

    fn id(&self) -> &'static str {
        "pg-dv"
    }

    fn path(&self) -> Vec<String> {
        vec![self.schema.clone(), self.table.clone()]
    }

    fn catalog(&self) -> Fut<HashMap<String, GridColumnMeta>> {
        let (pool, schema, table) = (self.pool.clone(), self.schema.clone(), self.table.clone());
        Box::pin(async move {
            column_catalog::load_postgres_column_catalog(&pool, &schema, &table).await
        })
    }

    fn page(
        &self,
        filter: Option<FilterExpr>,
        offset: u64,
        limit: u64,
        catalog: HashMap<String, GridColumnMeta>,
    ) -> Fut<PageData> {
        let pool = self.pool.clone();
        let table = format!("{}.{}", quote_ident(&self.schema), quote_ident(&self.table));
        let where_clause = filter
            .map(|e| format!(" WHERE {}", e.to_sql_postgres()))
            .unwrap_or_default();
        Box::pin(async move {
            let count_sql = format!("SELECT COUNT(*) FROM {table}{where_clause}");
            let total: i64 = sqlx::query_scalar(AssertSqlSafe(count_sql))
                .fetch_one(&pool)
                .await
                .unwrap_or(0);
            let fetch_sql =
                format!("SELECT * FROM {table}{where_clause} LIMIT {limit} OFFSET {offset}");
            let rows = sqlx::query(AssertSqlSafe(fetch_sql))
                .fetch_all(&pool)
                .await?;
            let names = rows
                .first()
                .map(|r| r.columns().iter().map(|c| c.name().to_string()).collect());
            let data = rows
                .iter()
                .map(|row| {
                    (0..row.len())
                        .map(|i| SharedString::from(pg_cell_display(row, i)))
                        .collect()
                })
                .collect();
            Ok(PageData::new(total, names, data, &catalog))
        })
    }
}
