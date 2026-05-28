//! MongoDB configuration, client helpers, and document mutations (no UI).

pub mod client;
pub mod config;
pub mod mutations;

pub use client::{apply_auth_source, resolve_database_name, test_database_name};
pub use config::MongoConfig;
pub use mutations::{delete_by_id, document_from_json, replace_by_id, update_fields_by_id};
