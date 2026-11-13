//! The objects a connection lists: collections.

use anyhow::Result;
use based_core::{DbObject, ObjectKind};
use mongodb::Database;

/// Every collection in the database, in the order the server returns them.
pub async fn list_collections(db: &Database) -> Result<Vec<DbObject>> {
    Ok(db
        .list_collection_names()
        .await?
        .into_iter()
        .map(|name| DbObject::new(None, name, ObjectKind::Collection))
        .collect())
}
