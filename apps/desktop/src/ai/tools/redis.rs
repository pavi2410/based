//! Redis as a tool backend. Only commands the read-only guard allows reach `query`.

use based_ai::AiError;
use based_ai::context::{Entity, EntityKind, Field};
use based_ai::tools::{QueryOutput, ToolBackend};
use based_core::EngineKind;
use based_redis::{Conn, exec, key_meta, scan_keys};
use futures::future::BoxFuture;

const MAX_KEYS: usize = 100;

use super::fail;

pub struct RedisTools(pub Conn);

impl ToolBackend for RedisTools {
    fn engine(&self) -> EngineKind {
        EngineKind::Redis
    }

    fn list_tables(&self) -> BoxFuture<'_, Result<Vec<String>, AiError>> {
        Box::pin(async move {
            let mut conn = self.0.clone();
            scan_keys(&mut conn, "*", MAX_KEYS).await.map_err(fail)
        })
    }

    fn describe_table<'a>(
        &'a self,
        table: &'a str,
    ) -> BoxFuture<'a, Result<Option<Entity>, AiError>> {
        Box::pin(async move {
            let mut conn = self.0.clone();
            let meta = key_meta(&mut conn, table).await.map_err(fail)?;
            let Some(kind) = meta.kind else {
                return Ok(None);
            };
            let field = |name: &str, ty: Option<String>| Field {
                name: name.into(),
                ty,
                ..Field::default()
            };
            let fields = vec![
                field("type", Some(kind.label().into())),
                field("ttl", meta.ttl.map(|t| format!("{t}s"))),
                field("length", meta.len.map(|n| n.to_string())),
            ];
            Ok(Some(Entity::new(table, EntityKind::KeyPattern, fields)))
        })
    }

    fn query<'a>(
        &'a self,
        query: &'a str,
        _limit: usize,
    ) -> BoxFuture<'a, Result<QueryOutput, AiError>> {
        Box::pin(async move {
            let mut conn = self.0.clone();
            let reply = exec(&mut conn, query).await.map_err(fail)?;
            Ok(QueryOutput {
                columns: vec!["reply".into()],
                rows: reply.lines().map(|l| vec![l.to_owned()]).collect(),
            })
        })
    }

    fn explain<'a>(&'a self, _: &'a str) -> BoxFuture<'a, Result<String, AiError>> {
        Box::pin(async { Err(fail("explain is not available for Redis")) })
    }
}
