//! AI assistant glue: streams provider output into GPUI entities.

mod ask;
pub mod chat;
mod host;
pub mod keys;
mod launch;
mod prompt;
pub mod schema;
mod status;
mod stream;
pub mod tools;

pub use ask::ask_assistant;
pub use host::{AiHost, OFF_NOTICE, connection_ai};
pub use launch::complete;
pub use prompt::AiPromptEvent;
pub use status::{AiStatus, init};
