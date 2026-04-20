//! MongoDB-specific engine operations: collection listing, collection
//! browse, raw find / aggregate execution, and BSON→JSON conversion.

use crate::error::Error;
use futures::TryStreamExt;
use mongodb::Database;
use mongodb::bson::{Bson, Document, doc};
use mongodb::options::FindOptions;
use serde::{Deserialize, Serialize};
use specta::Type;

use super::filters::{build_mongodb_filter, parse_filters};
use super::types::{
    BrowseOptions, ColumnDescription, ColumnInfo, IndexDescription, QueryResult, TableDescription,
};

#[derive(Debug, Serialize, Deserialize, Type)]
pub struct MongoDBCollection {
    pub name: String,
}

pub async fn list_collections(db: &Database) -> Result<Vec<MongoDBCollection>, Error> {
    let names = db.list_collection_names(None).await?;
    Ok(names
        .into_iter()
        .map(|name| MongoDBCollection { name })
        .collect())
}

pub async fn browse_collection(
    db: &Database,
    collection_name: &str,
    options: &BrowseOptions,
) -> Result<QueryResult, Error> {
    let collection = db.collection::<Document>(collection_name);

    let filters = parse_filters(options.filters.as_deref());
    let query_doc = build_mongodb_filter(&filters);

    let total_count = collection.count_documents(query_doc.clone(), None).await? as i64;

    let limit = options.limit.unwrap_or(100);
    let offset = options.offset.unwrap_or(0);

    let sort_doc = match (&options.order_by_column, &options.order_by_direction) {
        (Some(col), Some(dir)) => {
            let direction = if dir.eq_ignore_ascii_case("desc") {
                -1
            } else {
                1
            };
            Some(doc! { col.as_str(): direction })
        }
        _ => None,
    };
    let find_options = FindOptions::builder()
        .limit(limit)
        .skip(offset as u64)
        .sort(sort_doc)
        .build();

    let mut cursor = collection.find(query_doc, find_options).await?;
    let mut documents: Vec<Document> = Vec::new();
    while let Some(doc) = cursor.try_next().await? {
        documents.push(doc);
    }

    Ok(documents_to_result(documents, Some(total_count)))
}

/// Describe a MongoDB collection by sampling a handful of documents
/// and unioning their top-level fields. This is the best we can do in
/// a schemaless store — there is no catalog to ask.
///
/// Indexes are reported from `listIndexes` so users can at least see
/// what's there even though the column list is sampled.
pub async fn describe_collection(
    db: &Database,
    collection_name: &str,
) -> Result<TableDescription, Error> {
    let collection = db.collection::<Document>(collection_name);

    const SAMPLE_SIZE: i64 = 100;

    // Count is cheap enough on a typical collection; this is for the
    // inspector header, not the browse pagination, so we can eat the
    // cost.
    let row_count = collection
        .estimated_document_count(None)
        .await
        .ok()
        .map(|c| c as i64);

    // Sample the first N documents to derive columns. Using
    // `find().limit()` rather than `$sample` so small collections
    // deterministically show every field.
    let find_options = FindOptions::builder().limit(SAMPLE_SIZE).build();
    let mut cursor = collection.find(doc! {}, find_options).await?;
    let mut seen_types: std::collections::BTreeMap<String, String> =
        std::collections::BTreeMap::new();
    let mut order: Vec<String> = Vec::new();

    while let Some(d) = cursor.try_next().await? {
        for (k, v) in d.iter() {
            if !seen_types.contains_key(k) {
                order.push(k.clone());
            }
            let ty = bson_type_name(v);
            seen_types
                .entry(k.clone())
                .and_modify(|existing| {
                    if existing != &ty && existing != "mixed" {
                        *existing = "mixed".to_string();
                    }
                })
                .or_insert(ty);
        }
    }

    let columns: Vec<ColumnDescription> = order
        .iter()
        .enumerate()
        .map(|(i, name)| ColumnDescription {
            name: name.clone(),
            data_type: seen_types.get(name).cloned().unwrap_or_else(|| "mixed".into()),
            nullable: true,
            default: None,
            // `_id` is the only guaranteed PK in a Mongo collection.
            is_primary_key: name == "_id",
            position: (i + 1) as i32,
        })
        .collect();

    let mut indexes: Vec<IndexDescription> = Vec::new();
    let mut idx_cursor = collection.list_indexes(None).await?;
    while let Some(idx) = idx_cursor.try_next().await? {
        let opts = idx.options.unwrap_or_default();
        let name = opts.name.unwrap_or_default();
        let key_cols: Vec<String> = idx.keys.keys().cloned().collect();
        indexes.push(IndexDescription {
            unique: opts.unique.unwrap_or(false),
            primary: name == "_id_",
            name,
            columns: key_cols,
        });
    }

    Ok(TableDescription {
        name: collection_name.to_string(),
        schema: None,
        kind: "collection".to_string(),
        columns,
        indexes,
        foreign_keys: Vec::new(),
        row_count,
    })
}

fn bson_type_name(v: &Bson) -> String {
    match v {
        Bson::Null => "null",
        Bson::Boolean(_) => "bool",
        Bson::Int32(_) => "int32",
        Bson::Int64(_) => "int64",
        Bson::Double(_) => "double",
        Bson::String(_) => "string",
        Bson::ObjectId(_) => "objectId",
        Bson::DateTime(_) => "datetime",
        Bson::Array(_) => "array",
        Bson::Document(_) => "document",
        Bson::Binary(_) => "binary",
        Bson::Timestamp(_) => "timestamp",
        Bson::RegularExpression(_) => "regex",
        _ => "mixed",
    }
    .to_string()
}

pub async fn execute_raw(
    db: &Database,
    collection: &str,
    query_type: &str,
    query: &str,
) -> Result<QueryResult, Error> {
    let coll = db.collection::<Document>(collection);

    let documents: Vec<Document> = match query_type {
        "find" => {
            let filter: Document = serde_json::from_str(query)?;
            let mut cursor = coll.find(filter, None).await?;
            let mut docs = Vec::new();
            while let Some(doc) = cursor.try_next().await? {
                docs.push(doc);
            }
            docs
        }
        "aggregate" => {
            let pipeline: Vec<Document> = serde_json::from_str(query)?;
            let mut cursor = coll.aggregate(pipeline, None).await?;
            let mut docs = Vec::new();
            while let Some(doc) = cursor.try_next().await? {
                docs.push(doc);
            }
            docs
        }
        other => {
            return Err(Error::InvalidDbUrl(format!(
                "Unknown query type: {}. Use 'find' or 'aggregate'",
                other
            )));
        }
    };

    Ok(documents_to_result(documents, None))
}

fn documents_to_result(documents: Vec<Document>, total_count: Option<i64>) -> QueryResult {
    // MongoDB is schemaless, so we derive columns from the first doc.
    // Phase 1 should union keys across all rows so the UI doesn't hide
    // fields that appear later — tracked under the schema-inspector
    // todo.
    let columns: Vec<ColumnInfo> = documents
        .first()
        .map(|d| {
            d.keys()
                .map(|key| ColumnInfo {
                    name: key.clone(),
                    data_type: "mixed".to_string(),
                })
                .collect()
        })
        .unwrap_or_default();

    let rows: Vec<Vec<serde_json::Value>> = documents
        .iter()
        .map(|d| {
            columns
                .iter()
                .map(|col| {
                    d.get(&col.name)
                        .map(bson_to_json)
                        .unwrap_or(serde_json::Value::Null)
                })
                .collect()
        })
        .collect();

    let row_count = rows.len() as i64;
    QueryResult {
        columns,
        rows,
        total_count: total_count.or(Some(row_count)),
    }
}

pub fn bson_to_json(bson: &Bson) -> serde_json::Value {
    match bson {
        Bson::Null => serde_json::Value::Null,
        Bson::Boolean(b) => serde_json::Value::Bool(*b),
        Bson::Int32(i) => serde_json::Value::Number((*i).into()),
        Bson::Int64(i) => serde_json::Value::Number((*i).into()),
        Bson::Double(f) => serde_json::Number::from_f64(*f)
            .map(serde_json::Value::Number)
            .unwrap_or(serde_json::Value::Null),
        Bson::String(s) => serde_json::Value::String(s.clone()),
        Bson::ObjectId(oid) => serde_json::Value::String(oid.to_hex()),
        Bson::DateTime(dt) => serde_json::Value::String(dt.to_string()),
        Bson::Array(arr) => serde_json::Value::Array(arr.iter().map(bson_to_json).collect()),
        Bson::Document(d) => {
            let map: serde_json::Map<String, serde_json::Value> = d
                .iter()
                .map(|(k, v)| (k.clone(), bson_to_json(v)))
                .collect();
            serde_json::Value::Object(map)
        }
        Bson::Binary(bin) => {
            serde_json::Value::String(format!("[Binary: {} bytes]", bin.bytes.len()))
        }
        Bson::Timestamp(ts) => {
            serde_json::Value::String(format!("Timestamp({}, {})", ts.time, ts.increment))
        }
        Bson::RegularExpression(regex) => {
            serde_json::Value::String(format!("/{}/{}", regex.pattern, regex.options))
        }
        _ => serde_json::Value::String(bson.to_string()),
    }
}
