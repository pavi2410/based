//! LLM providers, schema context and tools for Based (no UI dependencies).

pub mod error;
pub mod provider;
pub mod types;

pub use error::{AiError, AiErrorKind};
pub use provider::{ChunkStream, Provider};
pub use types::{ChatRequest, Chunk, Message, Role, Usage};
