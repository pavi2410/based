// SQLite table inspector source: columns, indexes, constraints, DDL.

use futures::future::BoxFuture;
use sqlx::SqlitePool;

use crate::workspace::panels::table_inspector::{InspectSource, Kind, Section, TabDef};

pub struct SqliteInspect {
    pub pool: SqlitePool,
    pub table: String,
}

impl InspectSource for SqliteInspect {
    fn panel_name(&self) -> &'static str {
        "SqliteTableInspector"
    }

    fn breadcrumb_id(&self) -> &'static str {
        "sqlite-insp-breadcrumb"
    }

    fn path(&self) -> Vec<String> {
        vec![self.table.clone()]
    }

    fn tabs(&self) -> Vec<TabDef> {
        let tab = |id, label, kind| TabDef { id, label, kind };
        vec![
            tab("sql-insp-col", "Columns", Kind::Table),
            tab("sql-insp-ix", "Indexes", Kind::Table),
            tab("sql-insp-con", "Constraints", Kind::Table),
            tab("sql-insp-ddl", "DDL", Kind::Code),
        ]
    }

    fn load(&self) -> BoxFuture<'static, anyhow::Result<Vec<Section>>> {
        let (pool, table) = (self.pool.clone(), self.table.clone());
        Box::pin(async move {
            let info = based_sqlite::inspect_table(&pool, &table).await?;
            Ok(Section::from_info(info))
        })
    }
}
