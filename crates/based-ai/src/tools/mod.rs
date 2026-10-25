//! Read-only tools the model may call. Every query passes `guard` before it reaches a backend.

mod guard;
mod run;
mod spec;

pub use guard::check;
pub use run::{QueryOutput, ReadOnlyTools, ToolBackend, ToolLimits};
pub use spec::{
    CONNECT, LIST_CONNECTIONS, ToolCall, ToolResult, ToolSpec, connection_specs, specs,
};
