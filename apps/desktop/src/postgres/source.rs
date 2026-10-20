// Postgres table source for the shared data viewer (schema-qualified).

use std::collections::HashMap;

use based_core::quote_ident;
use gpui_kit::SharedString;
use sqlx::PgPool;

use crate::widgets::column_header::ColumnMeta;
use crate::widgets::filter_bar::FilterExpr;
use crate::workspace::panels::data_viewer::{DataSource, Fut, PageData};

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

    fn catalog(&self) -> Fut<HashMap<String, ColumnMeta>> {
        let (pool, schema, table) = (self.pool.clone(), self.schema.clone(), self.table.clone());
        Box::pin(async move { based_postgres::column_catalog(&pool, &schema, &table).await })
    }

    fn page(
        &self,
        filter: Option<FilterExpr>,
        offset: u64,
        limit: u64,
        catalog: HashMap<String, ColumnMeta>,
    ) -> Fut<PageData> {
        let pool = self.pool.clone();
        let table = format!("{}.{}", quote_ident(&self.schema), quote_ident(&self.table));
        let where_sql = filter
            .map(|e| format!(" WHERE {}", e.to_sql_postgres()))
            .unwrap_or_default();
        Box::pin(async move {
            let page = based_postgres::fetch_page(&pool, &table, &where_sql, limit, offset).await?;
            let rows = page
                .rows
                .into_iter()
                .map(|r| r.into_iter().map(SharedString::from).collect())
                .collect();
            Ok(PageData::new(page.total, page.columns, rows, &catalog))
        })
    }
}
