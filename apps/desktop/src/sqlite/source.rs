// SQLite table source for the shared data viewer.

use based_core::quote_ident;
use sqlx::SqlitePool;

use crate::widgets::filter_bar::FilterSet;
use crate::workspace::panels::data_viewer::{DataSource, Fut, PageData};
use crate::workspace::tabs::kind::names;
use based_core::ColumnCatalog;

pub struct SqliteSource {
    pub pool: SqlitePool,
    pub table: String,
}

impl DataSource for SqliteSource {
    fn panel_name(&self) -> &'static str {
        names::SQLITE_DATA_VIEWER
    }

    fn id(&self) -> &'static str {
        "sqlite-dv"
    }

    fn path(&self) -> Vec<String> {
        vec![self.table.clone()]
    }

    fn catalog(&self) -> Fut<ColumnCatalog> {
        let (pool, table) = (self.pool.clone(), self.table.clone());
        Box::pin(async move { based_sqlite::column_catalog(&pool, &table).await })
    }

    fn page(
        &self,
        filter: Option<FilterSet>,
        offset: u64,
        limit: u64,
        catalog: ColumnCatalog,
    ) -> Fut<PageData> {
        let pool = self.pool.clone();
        let table = quote_ident(&self.table);
        let where_sql = filter
            .map(|e| format!(" WHERE {}", e.to_sql_sqlite()))
            .unwrap_or_default();
        Box::pin(async move {
            let page = based_sqlite::fetch_page(&pool, &table, &where_sql, limit, offset).await?;
            Ok(PageData::new(page, &catalog))
        })
    }
}
