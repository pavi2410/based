//! MongoDB as a tool backend. Queries are `{"collection": ..., "pipeline": [...]}`.

use std::collections::BTreeMap;

use based_ai::AiError;
use based_ai::context::{Entity, EntityKind, Field};
use based_ai::tools::{QueryOutput, ToolBackend};
use based_core::EngineKind;
use futures::TryStreamExt;
use futures::future::BoxFuture;
use mongodb::Database;
use mongodb::bson::{Document, doc, to_document};
use serde_json::Value;

const SAMPLE_DOCS: i64 = 20;

use super::fail;

pub struct MongoTools(pub Database);

/// Splits a tool query into its collection and pipeline stages.
fn parse(query: &str) -> Result<(String, Vec<Document>), AiError> {
    let v: Value = serde_json::from_str(query)
        .map_err(|e| fail(format!("query must be JSON {{collection, pipeline}}: {e}")))?;
    let collection = v["collection"]
        .as_str()
        .ok_or_else(|| fail("missing string field `collection`"))?;
    let stages = v["pipeline"]
        .as_array()
        .ok_or_else(|| fail("missing array field `pipeline`"))?;
    let pipeline = stages
        .iter()
        .map(|s| to_document(s).map_err(fail))
        .collect::<Result<_, _>>()?;
    Ok((collection.to_owned(), pipeline))
}

impl ToolBackend for MongoTools {
    fn engine(&self) -> EngineKind {
        EngineKind::MongoDB
    }

    fn list_tables(&self) -> BoxFuture<'_, Result<Vec<String>, AiError>> {
        Box::pin(async move { self.0.list_collection_names().await.map_err(fail) })
    }

    fn describe_table<'a>(
        &'a self,
        table: &'a str,
    ) -> BoxFuture<'a, Result<Option<Entity>, AiError>> {
        Box::pin(async move {
            let coll = self.0.collection::<Document>(table);
            let mut cursor = coll.find(doc! {}).limit(SAMPLE_DOCS).await.map_err(fail)?;
            let mut types: BTreeMap<String, String> = BTreeMap::new();
            while let Some(d) = cursor.try_next().await.map_err(fail)? {
                for (key, value) in &d {
                    types
                        .entry(key.clone())
                        .or_insert_with(|| format!("{:?}", value.element_type()));
                }
            }
            let fields = types
                .into_iter()
                .map(|(name, ty)| Field {
                    name,
                    ty: Some(ty),
                    ..Field::default()
                })
                .collect();
            Ok(Some(Entity::new(table, EntityKind::Collection, fields)))
        })
    }

    fn query<'a>(
        &'a self,
        query: &'a str,
        limit: usize,
    ) -> BoxFuture<'a, Result<QueryOutput, AiError>> {
        Box::pin(async move {
            let (collection, mut pipeline) = parse(query)?;
            pipeline.push(doc! { "$limit": limit as i64 + 1 });
            let mut cursor = self
                .0
                .collection::<Document>(&collection)
                .aggregate(pipeline)
                .await
                .map_err(fail)?;
            let mut rows = Vec::new();
            while let Some(d) = cursor.try_next().await.map_err(fail)? {
                rows.push(vec![serde_json::to_string(&d).map_err(fail)?]);
            }
            Ok(QueryOutput {
                columns: vec!["document".into()],
                rows,
            })
        })
    }

    fn explain<'a>(&'a self, _: &'a str) -> BoxFuture<'a, Result<String, AiError>> {
        Box::pin(async { Err(fail("explain is not available for MongoDB")) })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_collection_and_pipeline() {
        let (c, p) = parse(r#"{"collection":"users","pipeline":[{"$match":{"a":1}}]}"#).unwrap();
        assert_eq!(c, "users");
        assert_eq!(p.len(), 1);
    }

    #[test]
    fn rejects_bare_pipelines() {
        assert!(parse(r#"[{"$match":{}}]"#).is_err());
    }
}
