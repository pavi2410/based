// SQLite table inspector source: columns, indexes, constraints, DDL.

use futures::future::BoxFuture;
use sqlx::SqlitePool;

use based_core::{ObjectRef, TableSpec};

use crate::workspace::panels::table_inspector::InspectSource;
use crate::workspace::tabs::kind::names;

pub struct SqliteInspect {
    pub pool: SqlitePool,
    pub table: String,
}

impl InspectSource for SqliteInspect {
    fn panel_name(&self) -> &'static str {
        names::SQLITE_TABLE_INSPECTOR
    }

    fn breadcrumb_id(&self) -> &'static str {
        "sqlite-insp-breadcrumb"
    }

    fn object(&self) -> ObjectRef {
        ObjectRef::bare(self.table.clone())
    }

    fn load(&self) -> BoxFuture<'static, anyhow::Result<TableSpec>> {
        let (pool, table) = (self.pool.clone(), self.table.clone());
        Box::pin(async move { based_sqlite::table_spec(&pool, &table).await })
    }
}
