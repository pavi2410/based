//! LLM providers, schema context and tools for Based (no UI dependencies).

pub mod context;
pub mod error;
mod http;
pub mod patch;
pub mod provider;
pub mod providers;
mod sse;
pub mod tools;
pub mod types;

pub use error::{AiError, AiErrorKind};
pub use provider::{ChunkStream, Provider};
pub use providers::{Anthropic, Ollama, OpenAi, ProviderKind};
pub use types::{ChatRequest, Chunk, Message, Role, Usage};
