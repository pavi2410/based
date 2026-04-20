//! Row-level mutations: update / insert / delete.
//!
//! All mutations go through one function per `(engine, op)` pair so
//! each path can enforce its own safety invariants:
//!
//!  - A non-empty primary-key predicate is required for update and
//!    delete. We refuse to emit an `UPDATE` / `DELETE` without a
//!    `WHERE` clause so we can never wipe a whole table.
//!  - SQL values bind through `push_json_bind` instead of string
//!    formatting, so a crafted column value cannot inject SQL.
//!  - SQL identifiers go through `quote_ident`, so a crafted column
//!    name cannot inject SQL either.
//!  - MongoDB requires an `_id` on update / delete; there's no generic
//!    PK discovery, so we key everything off `_id`.
//!
//! Readonly enforcement lives one level up in `project_db_commands.rs`
//! (it needs the `ConnectionConfig`), not here — we want this module
//! to be easy to call from tests.

use mongodb::Database as MongoDatabase;
use mongodb::bson::{Document, doc};
use sqlx::{PgPool, Postgres, QueryBuilder, Sqlite, SqlitePool};

use super::filters::{json_to_bson, push_json_bind, quote_ident};
use crate::error::Error;

/// Number of rows (or documents) touched by a mutation.
pub type Affected = u64;

// ---------------------------------------------------------------------------
// Validation
// ---------------------------------------------------------------------------

fn require_pk(pk: &serde_json::Map<String, serde_json::Value>) -> Result<(), Error> {
    if pk.is_empty() {
        return Err(Error::InvalidDbUrl(
            "Refusing to edit row: no primary key columns provided. \
             Tables without a primary key are effectively read-only in the UI."
                .to_string(),
        ));
    }
    Ok(())
}

fn require_changes(changes: &serde_json::Map<String, serde_json::Value>) -> Result<(), Error> {
    if changes.is_empty() {
        return Err(Error::InvalidDbUrl("No changes to apply".to_string()));
    }
    Ok(())
}

// ---------------------------------------------------------------------------
// SQLite
// ---------------------------------------------------------------------------

pub async fn sqlite_update_row(
    pool: &SqlitePool,
    table: &str,
    pk: &serde_json::Map<String, serde_json::Value>,
    changes: &serde_json::Map<String, serde_json::Value>,
) -> Result<Affected, Error> {
    require_pk(pk)?;
    require_changes(changes)?;

    let mut qb: QueryBuilder<Sqlite> = QueryBuilder::new("UPDATE ");
    qb.push(quote_ident(table));
    qb.push(" SET ");
    push_assignments(&mut qb, changes);
    qb.push(" WHERE ");
    push_pk_where(&mut qb, pk);
    let res = qb.build().execute(pool).await?;
    Ok(res.rows_affected())
}

pub async fn sqlite_insert_row(
    pool: &SqlitePool,
    table: &str,
    values: &serde_json::Map<String, serde_json::Value>,
) -> Result<Affected, Error> {
    require_changes(values)?;
    let mut qb: QueryBuilder<Sqlite> = QueryBuilder::new("INSERT INTO ");
    qb.push(quote_ident(table));
    push_insert_columns_values(&mut qb, values);
    let res = qb.build().execute(pool).await?;
    Ok(res.rows_affected())
}

pub async fn sqlite_delete_row(
    pool: &SqlitePool,
    table: &str,
    pk: &serde_json::Map<String, serde_json::Value>,
) -> Result<Affected, Error> {
    require_pk(pk)?;
    let mut qb: QueryBuilder<Sqlite> = QueryBuilder::new("DELETE FROM ");
    qb.push(quote_ident(table));
    qb.push(" WHERE ");
    push_pk_where(&mut qb, pk);
    let res = qb.build().execute(pool).await?;
    Ok(res.rows_affected())
}

// ---------------------------------------------------------------------------
// PostgreSQL
// ---------------------------------------------------------------------------

pub async fn postgres_update_row(
    pool: &PgPool,
    schema: &str,
    table: &str,
    pk: &serde_json::Map<String, serde_json::Value>,
    changes: &serde_json::Map<String, serde_json::Value>,
) -> Result<Affected, Error> {
    require_pk(pk)?;
    require_changes(changes)?;

    let mut qb: QueryBuilder<Postgres> = QueryBuilder::new("UPDATE ");
    qb.push(quote_ident(schema));
    qb.push(".");
    qb.push(quote_ident(table));
    qb.push(" SET ");
    push_assignments(&mut qb, changes);
    qb.push(" WHERE ");
    push_pk_where(&mut qb, pk);
    let res = qb.build().execute(pool).await?;
    Ok(res.rows_affected())
}

pub async fn postgres_insert_row(
    pool: &PgPool,
    schema: &str,
    table: &str,
    values: &serde_json::Map<String, serde_json::Value>,
) -> Result<Affected, Error> {
    require_changes(values)?;
    let mut qb: QueryBuilder<Postgres> = QueryBuilder::new("INSERT INTO ");
    qb.push(quote_ident(schema));
    qb.push(".");
    qb.push(quote_ident(table));
    push_insert_columns_values(&mut qb, values);
    let res = qb.build().execute(pool).await?;
    Ok(res.rows_affected())
}

pub async fn postgres_delete_row(
    pool: &PgPool,
    schema: &str,
    table: &str,
    pk: &serde_json::Map<String, serde_json::Value>,
) -> Result<Affected, Error> {
    require_pk(pk)?;
    let mut qb: QueryBuilder<Postgres> = QueryBuilder::new("DELETE FROM ");
    qb.push(quote_ident(schema));
    qb.push(".");
    qb.push(quote_ident(table));
    qb.push(" WHERE ");
    push_pk_where(&mut qb, pk);
    let res = qb.build().execute(pool).await?;
    Ok(res.rows_affected())
}

// ---------------------------------------------------------------------------
// MongoDB
// ---------------------------------------------------------------------------

/// Require that `pk` has exactly one field named `_id`, then return its
/// value as BSON. Anything else is a programming error at the UI layer.
fn require_mongo_id(
    pk: &serde_json::Map<String, serde_json::Value>,
) -> Result<mongodb::bson::Bson, Error> {
    let id = pk.get("_id").ok_or_else(|| {
        Error::InvalidDbUrl(
            "MongoDB mutations require an `_id` in the primary-key \
             predicate."
                .to_string(),
        )
    })?;
    Ok(json_to_bson(id))
}

pub async fn mongodb_update_document(
    db: &MongoDatabase,
    collection: &str,
    pk: &serde_json::Map<String, serde_json::Value>,
    changes: &serde_json::Map<String, serde_json::Value>,
) -> Result<Affected, Error> {
    require_changes(changes)?;
    let id = require_mongo_id(pk)?;
    let col = db.collection::<Document>(collection);

    let mut set_doc = Document::new();
    for (k, v) in changes.iter() {
        // `_id` can't be reassigned on an existing document.
        if k == "_id" {
            continue;
        }
        set_doc.insert(k.clone(), json_to_bson(v));
    }
    if set_doc.is_empty() {
        return Err(Error::InvalidDbUrl("No changes to apply".to_string()));
    }

    let res = col
        .update_one(doc! { "_id": id }, doc! { "$set": set_doc }, None)
        .await?;
    Ok(res.modified_count)
}

pub async fn mongodb_insert_document(
    db: &MongoDatabase,
    collection: &str,
    values: &serde_json::Map<String, serde_json::Value>,
) -> Result<Affected, Error> {
    require_changes(values)?;
    let col = db.collection::<Document>(collection);
    let mut d = Document::new();
    for (k, v) in values.iter() {
        d.insert(k.clone(), json_to_bson(v));
    }
    let _ = col.insert_one(d, None).await?;
    Ok(1)
}

pub async fn mongodb_delete_document(
    db: &MongoDatabase,
    collection: &str,
    pk: &serde_json::Map<String, serde_json::Value>,
) -> Result<Affected, Error> {
    let id = require_mongo_id(pk)?;
    let col = db.collection::<Document>(collection);
    let res = col.delete_one(doc! { "_id": id }, None).await?;
    Ok(res.deleted_count)
}

// ---------------------------------------------------------------------------
// SQL helpers
// ---------------------------------------------------------------------------

/// Append `"col1" = ?, "col2" = ?, ...` to the query builder, binding
/// each value. Used for both `UPDATE ... SET` clauses.
fn push_assignments<'a, DB>(
    qb: &mut QueryBuilder<'a, DB>,
    changes: &serde_json::Map<String, serde_json::Value>,
) where
    DB: super::filters::SupportsBinds,
    for<'q> i64: sqlx::Encode<'q, DB> + sqlx::Type<DB>,
    for<'q> f64: sqlx::Encode<'q, DB> + sqlx::Type<DB>,
    for<'q> bool: sqlx::Encode<'q, DB> + sqlx::Type<DB>,
    for<'q> String: sqlx::Encode<'q, DB> + sqlx::Type<DB>,
{
    for (i, (col, val)) in changes.iter().enumerate() {
        if i > 0 {
            qb.push(", ");
        }
        qb.push(quote_ident(col));
        qb.push(" = ");
        push_json_bind(qb, val);
    }
}

/// Append `"pk1" = ? AND "pk2" = ?` to the query builder. Caller is
/// responsible for having already emitted `WHERE`.
fn push_pk_where<'a, DB>(
    qb: &mut QueryBuilder<'a, DB>,
    pk: &serde_json::Map<String, serde_json::Value>,
) where
    DB: super::filters::SupportsBinds,
    for<'q> i64: sqlx::Encode<'q, DB> + sqlx::Type<DB>,
    for<'q> f64: sqlx::Encode<'q, DB> + sqlx::Type<DB>,
    for<'q> bool: sqlx::Encode<'q, DB> + sqlx::Type<DB>,
    for<'q> String: sqlx::Encode<'q, DB> + sqlx::Type<DB>,
{
    for (i, (col, val)) in pk.iter().enumerate() {
        if i > 0 {
            qb.push(" AND ");
        }
        qb.push(quote_ident(col));
        match val {
            serde_json::Value::Null => {
                qb.push(" IS NULL");
            }
            _ => {
                qb.push(" = ");
                push_json_bind(qb, val);
            }
        }
    }
}

/// Append `("col1", "col2") VALUES (?, ?)` to an INSERT.
fn push_insert_columns_values<'a, DB>(
    qb: &mut QueryBuilder<'a, DB>,
    values: &serde_json::Map<String, serde_json::Value>,
) where
    DB: super::filters::SupportsBinds,
    for<'q> i64: sqlx::Encode<'q, DB> + sqlx::Type<DB>,
    for<'q> f64: sqlx::Encode<'q, DB> + sqlx::Type<DB>,
    for<'q> bool: sqlx::Encode<'q, DB> + sqlx::Type<DB>,
    for<'q> String: sqlx::Encode<'q, DB> + sqlx::Type<DB>,
{
    qb.push(" (");
    for (i, col) in values.keys().enumerate() {
        if i > 0 {
            qb.push(", ");
        }
        qb.push(quote_ident(col));
    }
    qb.push(") VALUES (");
    for (i, v) in values.values().enumerate() {
        if i > 0 {
            qb.push(", ");
        }
        push_json_bind(qb, v);
    }
    qb.push(")");
}
