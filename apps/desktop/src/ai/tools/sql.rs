//! Postgres and SQLite as tool backends: schema from the editor loaders, queries read-only.

use based_ai::AiError;
use based_ai::context::Entity;
use based_ai::tools::{QueryOutput, ToolBackend};
use based_core::EngineKind;
use futures::future::BoxFuture;
use sqlx::{PgPool, SqlitePool};

use crate::ai::schema;
use crate::editor::{SchemaCache, pg_schema, sqlite_schema};

#[derive(Clone)]
pub enum SqlPool {
    Postgres(PgPool),
    Sqlite(SqlitePool),
}

use super::fail;

pub struct SqlTools(pub SqlPool);

impl SqlTools {
    async fn cache(&self) -> Result<SchemaCache, AiError> {
        match &self.0 {
            SqlPool::Postgres(p) => pg_schema::load_schema(p).await,
            SqlPool::Sqlite(p) => sqlite_schema::load_schema(p).await,
        }
        .map_err(fail)
    }

    async fn rows(&self, sql: &str, limit: usize) -> Result<QueryOutput, AiError> {
        let (columns, rows) = match &self.0 {
            SqlPool::Postgres(p) => based_postgres::query_readonly(p, sql, limit).await,
            SqlPool::Sqlite(p) => based_sqlite::query_readonly(p, sql, limit).await,
        }
        .map_err(fail)?;
        Ok(QueryOutput { columns, rows })
    }
}

impl ToolBackend for SqlTools {
    fn engine(&self) -> EngineKind {
        match self.0 {
            SqlPool::Postgres(_) => EngineKind::Postgres,
            SqlPool::Sqlite(_) => EngineKind::SQLite,
        }
    }

    fn list_tables(&self) -> BoxFuture<'_, Result<Vec<String>, AiError>> {
        Box::pin(async move {
            let cache = self.cache().await?;
            Ok(cache.objects.iter().map(|o| o.display()).collect())
        })
    }

    fn describe_table<'a>(
        &'a self,
        table: &'a str,
    ) -> BoxFuture<'a, Result<Option<Entity>, AiError>> {
        Box::pin(async move {
            let cache = self.cache().await?;
            Ok(cache.find_by_name(table).map(schema::entity))
        })
    }

    fn query<'a>(
        &'a self,
        query: &'a str,
        limit: usize,
    ) -> BoxFuture<'a, Result<QueryOutput, AiError>> {
        Box::pin(self.rows(query, limit))
    }

    fn explain<'a>(&'a self, query: &'a str) -> BoxFuture<'a, Result<String, AiError>> {
        Box::pin(async move {
            let prefix = match self.0 {
                SqlPool::Postgres(_) => "EXPLAIN",
                SqlPool::Sqlite(_) => "EXPLAIN QUERY PLAN",
            };
            let out = self.rows(&format!("{prefix} {query}"), 200).await?;
            // Plans are one text column (Postgres) or the last column (SQLite `detail`).
            let lines = out.rows.iter().filter_map(|r| r.last().map(String::as_str));
            Ok(lines.collect::<Vec<_>>().join("\n"))
        })
    }
}
