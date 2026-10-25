//! AI assistant glue: streams provider output into GPUI entities.

mod prompt;
pub mod schema;
mod status;
mod stream;

pub use prompt::{AiPrompt, AiPromptEvent};
pub use status::{AiStatus, init};
