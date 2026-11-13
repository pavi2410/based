// Postgres table source for the shared data viewer (schema-qualified).

use based_core::{PageSort, page_order_sql, quote_ident};
use sqlx::PgPool;

use crate::workspace::panels::data_viewer::{DataSource, Fut, PageData};
use crate::workspace::tabs::kind::names;
use based_core::ColumnCatalog;
use based_query::filter::FilterSet;

pub struct PgSource {
    pub pool: PgPool,
    pub schema: String,
    pub table: String,
}

impl DataSource for PgSource {
    fn panel_name(&self) -> &'static str {
        names::PG_DATA_VIEWER
    }

    fn id(&self) -> &'static str {
        "pg-dv"
    }

    fn path(&self) -> Vec<String> {
        vec![self.schema.clone(), self.table.clone()]
    }

    fn catalog(&self) -> Fut<ColumnCatalog> {
        let (pool, schema, table) = (self.pool.clone(), self.schema.clone(), self.table.clone());
        Box::pin(async move { based_postgres::column_catalog(&pool, &schema, &table).await })
    }

    fn page(
        &self,
        filter: Option<FilterSet>,
        sort: Option<PageSort>,
        offset: u64,
        limit: u64,
        catalog: ColumnCatalog,
    ) -> Fut<PageData> {
        let pool = self.pool.clone();
        let table = format!("{}.{}", quote_ident(&self.schema), quote_ident(&self.table));
        let where_sql = filter
            .map(|e| format!(" WHERE {}", e.to_sql_postgres()))
            .unwrap_or_default();
        let order_sql = page_order_sql(&catalog, sort.as_ref());
        Box::pin(async move {
            let page =
                based_postgres::fetch_page(&pool, &table, &where_sql, &order_sql, limit, offset)
                    .await?;
            Ok(PageData::new(page, &catalog))
        })
    }
}
