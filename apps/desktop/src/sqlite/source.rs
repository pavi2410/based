// SQLite table source for the shared data viewer.

use std::collections::HashMap;

use based_core::quote_ident;
use gpui_kit::SharedString;
use sqlx::SqlitePool;

use crate::widgets::column_header::ColumnMeta;
use crate::widgets::filter_bar::FilterExpr;
use crate::workspace::panels::data_viewer::{DataSource, Fut, PageData};

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

    fn catalog(&self) -> Fut<HashMap<String, ColumnMeta>> {
        let (pool, table) = (self.pool.clone(), self.table.clone());
        Box::pin(async move { based_sqlite::column_catalog(&pool, &table).await })
    }

    fn page(
        &self,
        filter: Option<FilterExpr>,
        offset: u64,
        limit: u64,
        catalog: HashMap<String, ColumnMeta>,
    ) -> Fut<PageData> {
        let pool = self.pool.clone();
        let table = quote_ident(&self.table);
        let where_sql = filter
            .map(|e| format!(" WHERE {}", e.to_sql_sqlite()))
            .unwrap_or_default();
        Box::pin(async move {
            let page = based_sqlite::fetch_page(&pool, &table, &where_sql, limit, offset).await?;
            let rows = page
                .rows
                .into_iter()
                .map(|r| r.into_iter().map(SharedString::from).collect())
                .collect();
            Ok(PageData::new(page.total, page.columns, rows, &catalog))
        })
    }
}
