// SQLite query source for the shared query editor.

use std::time::Duration;

use based_core::EngineKind;
use gpui_kit::component::Theme;
use gpui_kit::{AnyElement, IntoElement as _};
use sqlx::{AssertSqlSafe, Column, Row, SqlitePool, TypeInfo};
use tokio::time::timeout;

use super::eqp_parse::{EqpNode, parse_eqp};
use super::eqp_viewer::render_eqp_body;
use crate::editor::{SchemaCache, sqlite_schema};
use crate::workspace::panels::query_editor::{Explained, Fut, PlanView, QuerySource, RunOutput};
use based_sqlite::sqlite_cell_display;

pub struct SqliteQuery {
    pub pool: SqlitePool,
}

struct EqpPlan(Vec<EqpNode>);

impl PlanView for EqpPlan {
    fn render(&self, theme: &Theme) -> AnyElement {
        render_eqp_body("sqlite-inline-eqp", &self.0, theme).into_any_element()
    }
}

impl QuerySource for SqliteQuery {
    fn panel_name(&self) -> &'static str {
        "SqliteQueryEditor"
    }

    fn id(&self) -> &'static str {
        "sqlite"
    }

    fn engine(&self) -> EngineKind {
        EngineKind::SQLite
    }

    fn default_sql(&self) -> &'static str {
        "SELECT * FROM sqlite_master LIMIT 20"
    }

    fn load_schema(&self) -> Option<Fut<SchemaCache>> {
        let pool = self.pool.clone();
        Some(Box::pin(
            async move { sqlite_schema::load_schema(&pool).await },
        ))
    }

    /// Runs every statement in the script; the last non-empty result set wins.
    fn run(&self, sql: String, timeout_secs: u32) -> Fut<RunOutput> {
        let pool = self.pool.clone();
        Box::pin(async move {
            let limit = Duration::from_secs(timeout_secs as u64);
            let mut out = RunOutput::default();
            for (i, stmt) in based_query::statements_in_script(&sql).iter().enumerate() {
                let text = stmt.text(&sql);
                if text.is_empty() {
                    continue;
                }
                let fetch = sqlx::query(AssertSqlSafe(text.to_string())).fetch_all(&pool);
                let rows = match timeout(limit, fetch).await {
                    Err(_) => anyhow::bail!(
                        "Query timed out after {timeout_secs}s (statement {})",
                        i + 1
                    ),
                    Ok(Err(e)) => anyhow::bail!("Statement {} failed: {e}", i + 1),
                    Ok(Ok(rows)) => rows,
                };
                if let Some(first) = rows.first() {
                    out.columns = first
                        .columns()
                        .iter()
                        .map(|c| (c.name().to_string(), c.type_info().name().to_string()))
                        .collect();
                    out.rows = rows
                        .iter()
                        .map(|r| (0..r.len()).map(|c| sqlite_cell_display(r, c)).collect())
                        .collect();
                }
            }
            Ok(out)
        })
    }

    fn explain(&self, sql: String) -> Fut<Explained> {
        let pool = self.pool.clone();
        Box::pin(async move {
            let rows = sqlx::query(AssertSqlSafe(format!("EXPLAIN QUERY PLAN {sql}")))
                .fetch_all(&pool)
                .await?;
            let flat: Vec<(i64, i64, String)> = rows
                .iter()
                .map(|r| {
                    (
                        r.try_get("id").unwrap_or(0),
                        r.try_get("parent").unwrap_or(0),
                        r.try_get("detail").unwrap_or_default(),
                    )
                })
                .collect();
            let roots = parse_eqp(&flat);
            Ok(if roots.is_empty() {
                Explained::Text("(EXPLAIN QUERY PLAN returned no rows)".into())
            } else {
                Explained::Plan(Box::new(EqpPlan(roots)))
            })
        })
    }
}
