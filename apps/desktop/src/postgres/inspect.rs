// Postgres table inspector source: columns, indexes, constraints, stats.

use futures::future::BoxFuture;
use sqlx::PgPool;

use based_core::{ObjectRef, TableSpec};

use crate::workspace::panels::table_inspector::InspectSource;
use crate::workspace::tabs::kind::names;

pub struct PgInspect {
    pub pool: PgPool,
    pub schema: String,
    pub table: String,
}

impl InspectSource for PgInspect {
    fn panel_name(&self) -> &'static str {
        names::PG_TABLE_INSPECTOR
    }

    fn breadcrumb_id(&self) -> &'static str {
        "pg-insp-breadcrumb"
    }

    fn object(&self) -> ObjectRef {
        ObjectRef::qualified(self.schema.clone(), self.table.clone())
    }

    fn load(&self) -> BoxFuture<'static, anyhow::Result<TableSpec>> {
        let (pool, schema, table) = (self.pool.clone(), self.schema.clone(), self.table.clone());
        Box::pin(async move { based_postgres::table_spec(&pool, &schema, &table).await })
    }
}
