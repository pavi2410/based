// Postgres query source for the shared query editor.

use std::time::Duration;

use based_core::EngineKind;
use gpui_kit::component::{Theme, scroll::ScrollableElement as _};
use gpui_kit::{
    AnyElement, InteractiveElement as _, IntoElement as _, ParentElement as _, Styled as _, div, px,
};
use sqlx::{AssertSqlSafe, PgPool, Row};
use tokio::time::timeout;

use super::explain_plan::{PlanNode, parse_pg_explain_json, render_plan_node};
use crate::workspace::panels::query_editor::{Explained, Fut, PlanView, QuerySource, RunOutput};

pub struct PgQuery {
    pub pool: PgPool,
}

struct PgPlan(PlanNode);

impl PlanView for PgPlan {
    fn render(&self, theme: &Theme) -> AnyElement {
        div()
            .id("pg-inline-explain")
            .size_full()
            .overflow_y_scrollbar()
            .p(px(8.0))
            .child(render_plan_node(&self.0, 0, theme))
            .into_any_element()
    }
}

impl QuerySource for PgQuery {
    fn panel_name(&self) -> &'static str {
        "PgQueryEditor"
    }

    fn id(&self) -> &'static str {
        "pg"
    }

    fn engine(&self) -> EngineKind {
        EngineKind::Postgres
    }

    fn default_sql(&self) -> &'static str {
        "SELECT 1 AS one;"
    }

    fn run(&self, sql: String, timeout_secs: u32) -> Fut<RunOutput> {
        let pool = self.pool.clone();
        Box::pin(async move {
            let limit = Duration::from_secs(timeout_secs as u64);
            let (cols, rows, affected) = timeout(limit, based_postgres::execute_sql(&pool, &sql))
                .await
                .map_err(|_| anyhow::anyhow!("Query timed out after {timeout_secs}s"))??;
            Ok(RunOutput {
                columns: cols.into_iter().map(|c| (c.name, c.type_name)).collect(),
                rows,
                affected: Some(affected),
            })
        })
    }

    fn explain(&self, sql: String) -> Fut<Explained> {
        let pool = self.pool.clone();
        Box::pin(async move {
            let rows = sqlx::query(AssertSqlSafe(format!("EXPLAIN (FORMAT JSON) {sql}")))
                .fetch_all(&pool)
                .await?;
            // `json` columns (OID 114) fail sqlx's `String` type check; read the wire text.
            let Some(row) = rows.first() else {
                return Ok(Explained::Text("(EXPLAIN returned no rows)".into()));
            };
            let raw: String = row.try_get_unchecked(0)?;
            Ok(match serde_json::from_str::<serde_json::Value>(&raw) {
                Ok(json) => match parse_pg_explain_json(&json) {
                    Some(plan) => Explained::Plan(Box::new(PgPlan(plan))),
                    None => Explained::Text(raw),
                },
                Err(e) => Explained::Text(format!("(invalid JSON: {e})\n\n{raw}")),
            })
        })
    }
}
