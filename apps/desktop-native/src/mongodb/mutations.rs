// mongodb::mutations — simple _id-targeted writes.

use anyhow::{Context, Result};
use mongodb::Collection;
use mongodb::bson::{Bson, Document, doc};

pub async fn delete_by_id(coll: &Collection<Document>, id: &str) -> Result<u64> {
    let oid = mongodb::bson::oid::ObjectId::parse_str(id)
        .map(Bson::ObjectId)
        .unwrap_or(Bson::String(id.to_string()));
    let r = coll.delete_one(doc! { "_id": oid }, None).await?;
    Ok(r.deleted_count)
}

pub async fn replace_by_id(
    coll: &Collection<Document>,
    id: &str,
    replacement: Document,
) -> Result<u64> {
    let oid = mongodb::bson::oid::ObjectId::parse_str(id)
        .map(Bson::ObjectId)
        .unwrap_or(Bson::String(id.to_string()));
    let filter = doc! { "_id": oid };
    let r = coll.replace_one(filter, replacement, None).await?;
    Ok(r.modified_count)
}

pub async fn update_fields_by_id(
    coll: &Collection<Document>,
    id: &str,
    set: Document,
) -> Result<u64> {
    let oid = mongodb::bson::oid::ObjectId::parse_str(id)
        .map(Bson::ObjectId)
        .unwrap_or(Bson::String(id.to_string()));
    let r = coll
        .update_one(doc! { "_id": oid }, doc! { "$set": set }, None)
        .await?;
    Ok(r.modified_count)
}

pub fn document_from_json(s: &str) -> Result<Document> {
    let v: serde_json::Value = serde_json::from_str(s).context("invalid JSON for document")?;
    Ok(mongodb::bson::to_document(&v)?)
}
