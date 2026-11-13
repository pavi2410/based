// SQLite query source for the shared query editor.

use std::time::Duration;

use based_core::{EngineKind, QueryError, SqlOutcome};
use futures::future::BoxFuture;
use gpui_kit::component::Theme;
use gpui_kit::{AnyElement, IntoElement as _};
use sqlx::{AssertSqlSafe, Row, SqlitePool};
use tokio::time::timeout;

use super::eqp_parse::{EqpNode, parse_eqp};
use super::eqp_viewer::render_eqp_body;
use crate::editor::SchemaCache;
use crate::editor::error_locate::locate_error;
use crate::workspace::panels::query_editor::{Explained, Fut, PlanView, QuerySource};
use crate::workspace::tabs::kind::names;

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
        names::SQLITE_QUERY_EDITOR
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
        Some(Box::pin(async move {
            Ok(SchemaCache::loaded(
                EngineKind::SQLite,
                based_sqlite::list_relations(&pool).await?,
            ))
        }))
    }

    /// Runs every statement in the script: the last row result wins, and with
    /// none, the writes' affected counts add up.
    fn run(
        &self,
        sql: String,
        timeout_secs: u32,
    ) -> BoxFuture<'static, Result<SqlOutcome, QueryError>> {
        let pool = self.pool.clone();
        Box::pin(async move {
            let limit = Duration::from_secs(timeout_secs as u64);
            let mut rows = None;
            let mut affected = 0u64;
            let statements = based_query::split_statements(&sql, EngineKind::SQLite);
            for (i, stmt) in statements.into_iter().enumerate() {
                let run = based_sqlite::execute_sql(&pool, &sql[stmt.clone()]);
                match timeout(limit, run).await {
                    Err(_) => {
                        return Err(QueryError::new(format!(
                            "Query timed out after {timeout_secs}s (statement {})",
                            i + 1
                        )));
                    }
                    Ok(Err(e)) => {
                        // SQLite reports no position; the `near "…"` token it names,
                        // looked up inside the statement that failed, is the best guess.
                        let message = e.to_string();
                        let position = locate_error(&sql[stmt.clone()], &message)
                            .map(|r| stmt.start + r.start);
                        return Err(QueryError {
                            message: format!("Statement {} failed: {message}", i + 1),
                            position,
                            connection_lost: false,
                        });
                    }
                    Ok(Ok(SqlOutcome::Rows(set))) => rows = Some(set),
                    Ok(Ok(SqlOutcome::Affected(n))) => affected += n,
                }
            }
            Ok(rows.map_or(SqlOutcome::Affected(affected), SqlOutcome::Rows))
        })
    }

    fn run_read_only(
        &self,
        sql: String,
        limit: usize,
    ) -> BoxFuture<'static, Result<SqlOutcome, QueryError>> {
        let pool = self.pool.clone();
        Box::pin(async move {
            based_sqlite::query_readonly(&pool, &sql, limit)
                .await
                .map(SqlOutcome::Rows)
                .map_err(|e| QueryError::new(e.to_string()))
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
