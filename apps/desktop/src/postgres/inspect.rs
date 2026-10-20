// Postgres table inspector source: columns, indexes, constraints, stats.

use futures::future::BoxFuture;
use sqlx::PgPool;

use crate::workspace::panels::table_inspector::{InspectSource, Kind, Section, TabDef};

pub struct PgInspect {
    pub pool: PgPool,
    pub schema: String,
    pub table: String,
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
            let info = based_postgres::inspect_table(&pool, &schema, &table).await?;
            Ok(Section::from_info(info))
        })
    }
}
