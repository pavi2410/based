//! AI assistant glue: streams provider output into GPUI entities.

mod status;
mod stream;

pub use status::{AiStatus, init};
