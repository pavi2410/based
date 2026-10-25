//! AI assistant glue: streams provider output into GPUI entities.

pub mod schema;
mod status;
mod stream;

pub use status::{AiStatus, init};
