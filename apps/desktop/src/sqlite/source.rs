// SQLite table source for the shared data viewer.

use std::collections::HashMap;

use gpui_kit::SharedString;
use sqlx::{AssertSqlSafe, Column, Row, SqlitePool};

use crate::db::column_catalog;
use crate::widgets::column_header::GridColumnMeta;
use crate::widgets::filter_bar::FilterExpr;
use crate::widgets::row_cell::sqlite_cell_display;
use crate::workspace::panels::data_viewer::{DataSource, Fut, PageData, quote_ident};

pub struct SqliteSource {
    pub pool: SqlitePool,
    pub table: String,
}

impl DataSource for SqliteSource {
    fn panel_name(&self) -> &'static str {
        "SqliteDataViewer"
    }

    fn id(&self) -> &'static str {
        "sqlite-dv"
    }

    fn path(&self) -> Vec<String> {
        vec![self.table.clone()]
    }

    fn catalog(&self) -> Fut<HashMap<String, GridColumnMeta>> {
        let (pool, table) = (self.pool.clone(), self.table.clone());
        Box::pin(async move { column_catalog::load_sqlite_column_catalog(&pool, &table).await })
    }

    fn page(
        &self,
        filter: Option<FilterExpr>,
        offset: u64,
        limit: u64,
        catalog: HashMap<String, GridColumnMeta>,
    ) -> Fut<PageData> {
        let pool = self.pool.clone();
        let table = quote_ident(&self.table);
        let where_clause = filter
            .map(|e| format!(" WHERE {}", e.to_sql_sqlite()))
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
                        .map(|i| SharedString::from(sqlite_cell_display(row, i)))
                        .collect()
                })
                .collect();
            Ok(PageData::new(total, names, data, &catalog))
        })
    }
}
