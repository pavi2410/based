//! AI assistant glue: streams provider output into GPUI entities.

mod host;
mod prompt;
pub mod schema;
mod status;
mod stream;

pub use host::{AiHost, connection_ai};
pub use prompt::AiPromptEvent;
pub use status::{AiStatus, init};
