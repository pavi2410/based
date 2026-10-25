//! AI assistant glue: streams provider output into GPUI entities.

pub mod chat;
mod host;
mod launch;
mod prompt;
pub mod schema;
mod status;
mod stream;

pub use host::{AiHost, OFF_NOTICE, connection_ai};
pub use prompt::AiPromptEvent;
pub use status::{AiStatus, init};
