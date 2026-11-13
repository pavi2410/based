use serde::{Deserialize, Serialize};

use crate::tools::{ToolCall, ToolResult, ToolSpec};

/// Who a message is from.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum Role {
    User,
    Assistant,
    /// The result of a tool call the assistant requested.
    Tool,
}

/// One message in a chat. Each kind carries only what it can have: tool calls
/// on assistant turns, a call id and result on tool turns.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(from = "RawMessage", into = "RawMessage")]
pub struct Message {
    pub body: MessageBody,
    /// Unix milliseconds when the message was sent or finished.
    pub at: Option<u64>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum MessageBody {
    User {
        content: String,
    },
    Assistant {
        content: String,
        /// Tools the assistant asked to run.
        calls: Vec<ToolCall>,
    },
    /// The answer to the call `call_id` (named `name`).
    Tool {
        call_id: String,
        name: String,
        result: ToolResult,
    },
}

impl Message {
    fn new(body: MessageBody) -> Self {
        Self { body, at: None }
    }

    /// Records when the message was sent or finished.
    pub fn at(mut self, ms: u64) -> Self {
        self.at = Some(ms);
        self
    }

    pub fn user(content: impl Into<String>) -> Self {
        Self::new(MessageBody::User {
            content: content.into(),
        })
    }

    pub fn assistant(content: impl Into<String>) -> Self {
        Self::assistant_calls(content, Vec::new())
    }

    /// An assistant turn that ends in tool calls.
    pub fn assistant_calls(content: impl Into<String>, calls: Vec<ToolCall>) -> Self {
        Self::new(MessageBody::Assistant {
            content: content.into(),
            calls,
        })
    }

    /// The answer to `call`.
    pub fn tool(call: &ToolCall, result: ToolResult) -> Self {
        Self::new(MessageBody::Tool {
            call_id: call.id.clone(),
            name: call.name.clone(),
            result,
        })
    }

    pub fn role(&self) -> Role {
        match self.body {
            MessageBody::User { .. } => Role::User,
            MessageBody::Assistant { .. } => Role::Assistant,
            MessageBody::Tool { .. } => Role::Tool,
        }
    }

    /// The text: what was said, or the tool's output.
    pub fn content(&self) -> &str {
        match &self.body {
            MessageBody::User { content } | MessageBody::Assistant { content, .. } => content,
            MessageBody::Tool { result, .. } => &result.content,
        }
    }

    /// Tool calls an assistant turn asked for (empty otherwise).
    pub fn calls(&self) -> &[ToolCall] {
        match &self.body {
            MessageBody::Assistant { calls, .. } => calls,
            _ => &[],
        }
    }
}

/// The stored shape of a [`Message`] (chat history files), unchanged from when
/// it was the in-memory type, so existing chats load and older builds read new ones.
#[derive(Serialize, Deserialize)]
struct RawMessage {
    role: Role,
    content: String,
    #[serde(default)]
    calls: Vec<ToolCall>,
    #[serde(default)]
    outcome: Option<RawToolOutcome>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    at: Option<u64>,
}

#[derive(Serialize, Deserialize)]
struct RawToolOutcome {
    id: String,
    name: String,
    is_error: bool,
}

impl From<RawMessage> for Message {
    fn from(raw: RawMessage) -> Self {
        let body = match raw.role {
            Role::User => MessageBody::User {
                content: raw.content,
            },
            Role::Assistant => MessageBody::Assistant {
                content: raw.content,
                calls: raw.calls,
            },
            Role::Tool => {
                let outcome = raw.outcome.unwrap_or(RawToolOutcome {
                    id: String::new(),
                    name: "tool".into(),
                    is_error: false,
                });
                MessageBody::Tool {
                    call_id: outcome.id,
                    name: outcome.name,
                    result: ToolResult {
                        content: raw.content,
                        is_error: outcome.is_error,
                    },
                }
            }
        };
        Self { body, at: raw.at }
    }
}

impl From<Message> for RawMessage {
    fn from(m: Message) -> Self {
        let role = m.role();
        let (content, calls, outcome) = match m.body {
            MessageBody::User { content } => (content, Vec::new(), None),
            MessageBody::Assistant { content, calls } => (content, calls, None),
            MessageBody::Tool {
                call_id,
                name,
                result,
            } => (
                result.content,
                Vec::new(),
                Some(RawToolOutcome {
                    id: call_id,
                    name,
                    is_error: result.is_error,
                }),
            ),
        };
        Self {
            role,
            content,
            calls,
            outcome,
            at: m.at,
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

#[cfg(test)]
mod message_tests {
    use super::*;

    #[test]
    fn old_chat_json_round_trips() {
        let old = r#"[
            {"role":"User","content":"q","calls":[],"outcome":null,"at":5},
            {"role":"Assistant","content":"","calls":[{"id":"c1","name":"list_tables","args":{}}],"outcome":null},
            {"role":"Tool","content":"t1","calls":[],"outcome":{"id":"c1","name":"list_tables","is_error":true}}
        ]"#;
        let msgs: Vec<Message> = serde_json::from_str(old).unwrap();
        assert_eq!(msgs[0].at, Some(5));
        assert_eq!(msgs[1].calls().len(), 1);
        assert_eq!(
            msgs[2].body,
            MessageBody::Tool {
                call_id: "c1".into(),
                name: "list_tables".into(),
                result: ToolResult {
                    content: "t1".into(),
                    is_error: true
                },
            }
        );
        let again: Vec<Message> =
            serde_json::from_str(&serde_json::to_string(&msgs).unwrap()).unwrap();
        assert_eq!(again, msgs);
    }
}
