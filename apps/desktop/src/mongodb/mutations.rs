//! Re-export MongoDB document mutations from `based-mongo`.

pub use based_mongo::{
    delete_by_id, document_from_json, replace_by_id, update_fields_by_id,
};
