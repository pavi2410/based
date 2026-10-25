use crate::tools::{ToolCall, ToolResult, ToolSpec};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Role {
    User,
    Assistant,
    /// The result of a tool call the assistant requested.
    Tool,
}

/// Which call a [`Role::Tool`] message answers.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ToolOutcome {
    pub id: String,
    pub name: String,
    pub is_error: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Message {
    pub role: Role,
    pub content: String,
    /// Tools the assistant asked to run (assistant messages only).
    pub calls: Vec<ToolCall>,
    /// Set on tool results.
    pub outcome: Option<ToolOutcome>,
}

impl Message {
    fn plain(role: Role, content: String) -> Self {
        Self {
            role,
            content,
            calls: Vec::new(),
            outcome: None,
        }
    }

    pub fn user(content: impl Into<String>) -> Self {
        Self::plain(Role::User, content.into())
    }

    pub fn assistant(content: impl Into<String>) -> Self {
        Self::plain(Role::Assistant, content.into())
    }

    /// An assistant turn that ends in tool calls.
    pub fn assistant_calls(content: impl Into<String>, calls: Vec<ToolCall>) -> Self {
        Self {
            calls,
            ..Self::plain(Role::Assistant, content.into())
        }
    }

    /// The answer to `call`.
    pub fn tool(call: &ToolCall, result: ToolResult) -> Self {
        Self {
            outcome: Some(ToolOutcome {
                id: call.id.clone(),
                name: call.name.clone(),
                is_error: result.is_error,
            }),
            ..Self::plain(Role::Tool, result.content)
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct ChatRequest {
    pub model: String,
    pub system: Option<String>,
    pub messages: Vec<Message>,
    pub tools: Vec<ToolSpec>,
    pub max_tokens: u32,
    pub temperature: Option<f32>,
}

impl ChatRequest {
    pub fn new(model: impl Into<String>, messages: Vec<Message>) -> Self {
        Self {
            model: model.into(),
            system: None,
            messages,
            tools: Vec::new(),
            max_tokens: 2048,
            temperature: None,
        }
    }

    pub fn with_system(mut self, system: impl Into<String>) -> Self {
        self.system = Some(system.into());
        self
    }

    pub fn with_tools(mut self, tools: Vec<ToolSpec>) -> Self {
        self.tools = tools;
        self
    }
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct Usage {
    pub input_tokens: u32,
    pub output_tokens: u32,
}

/// One streamed event from a provider.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Chunk {
    Text(String),
    /// A complete tool call (arguments fully received).
    ToolCall(ToolCall),
    Done(Usage),
}
