use std::fmt::Write;

use based_core::EngineKind;
use futures::future::BoxFuture;

use super::guard::check;
use super::spec::{ToolCall, ToolResult, ToolSpec, specs};
use crate::context::{ContextLevel, Entity, RenderOptions, SchemaSnapshot, render};
use crate::error::AiError;

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct QueryOutput {
    pub columns: Vec<String>,
    pub rows: Vec<Vec<String>>,
}

/// Engine access for tools. `query` and `explain` must run in a read-only
/// transaction/connection and respect `limit`.
pub trait ToolBackend: Send + Sync {
    fn engine(&self) -> EngineKind;
    fn list_tables(&self) -> BoxFuture<'_, Result<Vec<String>, AiError>>;
    fn describe_table<'a>(
        &'a self,
        table: &'a str,
    ) -> BoxFuture<'a, Result<Option<Entity>, AiError>>;
    fn query<'a>(
        &'a self,
        query: &'a str,
        limit: usize,
    ) -> BoxFuture<'a, Result<QueryOutput, AiError>>;
    fn explain<'a>(&'a self, query: &'a str) -> BoxFuture<'a, Result<String, AiError>>;
}

#[derive(Debug, Clone, Copy)]
pub struct ToolLimits {
    pub max_rows: usize,
    pub max_cell_chars: usize,
}

impl Default for ToolLimits {
    fn default() -> Self {
        Self {
            max_rows: 100,
            max_cell_chars: 200,
        }
    }
}

pub struct ReadOnlyTools<B> {
    backend: B,
    limits: ToolLimits,
}

impl<B: ToolBackend> ReadOnlyTools<B> {
    pub fn new(backend: B) -> Self {
        Self {
            backend,
            limits: ToolLimits::default(),
        }
    }

    pub fn with_limits(mut self, limits: ToolLimits) -> Self {
        self.limits = limits;
        self
    }

    pub fn specs(&self) -> Vec<ToolSpec> {
        specs()
    }

    pub async fn call(&self, call: &ToolCall) -> ToolResult {
        match self.dispatch(call).await {
            Ok(text) => ToolResult::ok(text),
            Err(msg) => ToolResult::err(msg),
        }
    }

    async fn dispatch(&self, call: &ToolCall) -> Result<String, String> {
        let arg = |k: &str| {
            call.args[k]
                .as_str()
                .map(str::to_owned)
                .ok_or_else(|| format!("missing string argument `{k}`"))
        };
        match call.name.as_str() {
            "list_tables" => {
                let names = self.backend.list_tables().await.map_err(|e| e.message)?;
                Ok(names.join("\n"))
            }
            "describe_table" => {
                let table = arg("table")?;
                let entity = self
                    .backend
                    .describe_table(&table)
                    .await
                    .map_err(|e| e.message)?;
                let entity = entity.ok_or_else(|| format!("no such table `{table}`"))?;
                let snap = SchemaSnapshot {
                    entities: vec![entity],
                };
                let opts = RenderOptions::new(self.backend.engine(), ContextLevel::Schema);
                Ok(render(&snap, &opts))
            }
            "explain" => {
                let q = arg("query")?;
                check(self.backend.engine(), &q)?;
                self.backend.explain(&q).await.map_err(|e| e.message)
            }
            "run_readonly_query" => {
                let q = arg("query")?;
                check(self.backend.engine(), &q)?;
                let out = self
                    .backend
                    .query(&q, self.limits.max_rows)
                    .await
                    .map_err(|e| e.message)?;
                Ok(format_output(&out, &self.limits))
            }
            other => Err(format!("unknown tool `{other}`")),
        }
    }
}

fn format_output(out: &QueryOutput, limits: &ToolLimits) -> String {
    let mut s = out.columns.join(" | ");
    s.push('\n');
    for row in out.rows.iter().take(limits.max_rows) {
        let cells: Vec<String> = row.iter().map(|c| clip(c, limits.max_cell_chars)).collect();
        let _ = writeln!(s, "{}", cells.join(" | "));
    }
    if out.rows.len() > limits.max_rows {
        let _ = writeln!(s, "... truncated at {} rows", limits.max_rows);
    }
    s
}

fn clip(cell: &str, max: usize) -> String {
    if cell.chars().count() <= max {
        return cell.to_owned();
    }
    let head: String = cell.chars().take(max).collect();
    format!("{head}...")
}

#[cfg(test)]
mod tests {
    use std::sync::atomic::{AtomicUsize, Ordering};

    use futures::FutureExt;
    use serde_json::json;

    use super::*;
    use crate::context::{EntityKind, Field};

    #[derive(Default)]
    struct Fake {
        queries: AtomicUsize,
    }

    impl ToolBackend for Fake {
        fn engine(&self) -> EngineKind {
            EngineKind::Postgres
        }
        fn list_tables(&self) -> BoxFuture<'_, Result<Vec<String>, AiError>> {
            async { Ok(vec!["users".into(), "orders".into()]) }.boxed()
        }
        fn describe_table<'a>(
            &'a self,
            t: &'a str,
        ) -> BoxFuture<'a, Result<Option<Entity>, AiError>> {
            async move {
                Ok((t == "users").then(|| {
                    let f = Field {
                        name: "id".into(),
                        ty: Some("int".into()),
                        ..Field::default()
                    };
                    Entity::new("users", EntityKind::Table, vec![f])
                }))
            }
            .boxed()
        }
        fn query<'a>(
            &'a self,
            _q: &'a str,
            _l: usize,
        ) -> BoxFuture<'a, Result<QueryOutput, AiError>> {
            self.queries.fetch_add(1, Ordering::SeqCst);
            async {
                Ok(QueryOutput {
                    columns: vec!["n".into()],
                    rows: (0..150).map(|i| vec![i.to_string()]).collect(),
                })
            }
            .boxed()
        }
        fn explain<'a>(&'a self, _q: &'a str) -> BoxFuture<'a, Result<String, AiError>> {
            async { Ok("Seq Scan".into()) }.boxed()
        }
    }

    fn call(name: &str, args: serde_json::Value) -> ToolCall {
        ToolCall {
            id: "1".into(),
            name: name.into(),
            args,
        }
    }

    #[tokio::test]
    async fn writes_never_reach_the_backend() {
        let tools = ReadOnlyTools::new(Fake::default());
        let r = tools
            .call(&call(
                "run_readonly_query",
                json!({ "query": "DELETE FROM users" }),
            ))
            .await;
        assert!(r.is_error);
        let r = tools
            .call(&call(
                "explain",
                json!({ "query": "EXPLAIN ANALYZE SELECT 1" }),
            ))
            .await;
        assert!(r.is_error);
        assert_eq!(tools.backend.queries.load(Ordering::SeqCst), 0);
    }

    #[tokio::test]
    async fn query_output_is_truncated() {
        let tools = ReadOnlyTools::new(Fake::default());
        let r = tools
            .call(&call(
                "run_readonly_query",
                json!({ "query": "SELECT n FROM t" }),
            ))
            .await;
        assert!(!r.is_error);
        assert!(r.content.contains("truncated at 100 rows"));
        assert_eq!(r.content.lines().count(), 102);
    }

    #[tokio::test]
    async fn describe_and_errors() {
        let tools = ReadOnlyTools::new(Fake::default());
        let r = tools
            .call(&call("describe_table", json!({ "table": "users" })))
            .await;
        assert_eq!(r.content, "users(id int)\n");
        assert!(
            tools
                .call(&call("describe_table", json!({ "table": "nope" })))
                .await
                .is_error
        );
        assert!(
            tools
                .call(&call("describe_table", json!({})))
                .await
                .is_error
        );
        assert!(tools.call(&call("drop_all", json!({}))).await.is_error);
    }
}
