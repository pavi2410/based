use futures::{StreamExt, future};
use reqwest::Client;
use serde_json::{Value, json};

use crate::error::{AiError, AiErrorKind};
use crate::http::{lazy_stream, sse_events};
use crate::provider::{ChunkStream, Provider};
use crate::sse::SseEvent;
use crate::tools::{ToolCall, ToolSpec};
use crate::types::{ChatRequest, Chunk, Message, Role, Usage};

const DEFAULT_URL: &str = "https://api.anthropic.com";
const VERSION: &str = "2023-06-01";

pub struct Anthropic {
    client: Client,
    api_key: String,
    base_url: String,
}

impl Anthropic {
    pub fn new(api_key: impl Into<String>) -> Self {
        Self {
            client: Client::new(),
            api_key: api_key.into(),
            base_url: DEFAULT_URL.into(),
        }
    }

    pub fn with_base_url(mut self, url: impl Into<String>) -> Self {
        self.base_url = url.into().trim_end_matches('/').to_owned();
        self
    }
}

impl Provider for Anthropic {
    fn name(&self) -> &'static str {
        "anthropic"
    }

    fn stream(&self, request: ChatRequest) -> ChunkStream {
        let req = self
            .client
            .post(format!("{}/v1/messages", self.base_url))
            .header("x-api-key", &self.api_key)
            .header("anthropic-version", VERSION)
            .json(&body(&request));
        lazy_stream(req, |resp| {
            let mut state = StreamState::default();
            sse_events(resp)
                .filter_map(move |ev| {
                    future::ready(match ev {
                        Ok(ev) => state.handle(&ev),
                        Err(e) => Some(Err(e)),
                    })
                })
                .boxed()
        })
    }
}

fn body(r: &ChatRequest) -> Value {
    let mut v = json!({
        "model": r.model,
        "max_tokens": r.max_tokens,
        "stream": true,
        "messages": wire_messages(&r.messages),
    });
    if !r.tools.is_empty() {
        v["tools"] = r.tools.iter().map(wire_tool).collect();
    }
    if let Some(s) = &r.system {
        v["system"] = json!(s);
    }
    if let Some(t) = r.temperature {
        v["temperature"] = json!(t);
    }
    v
}

fn wire_tool(t: &ToolSpec) -> Value {
    json!({ "name": t.name, "description": t.description, "input_schema": t.schema })
}

fn wire_messages(messages: &[Message]) -> Vec<Value> {
    messages
        .iter()
        .map(|m| match (&m.role, &m.outcome) {
            (Role::Tool, Some(o)) => json!({
                "role": "user",
                "content": [{
                    "type": "tool_result",
                    "tool_use_id": o.id,
                    "content": m.content,
                    "is_error": o.is_error,
                }],
            }),
            (Role::Assistant, _) if !m.calls.is_empty() => {
                let mut blocks = Vec::new();
                if !m.content.is_empty() {
                    blocks.push(json!({ "type": "text", "text": m.content }));
                }
                blocks.extend(m.calls.iter().map(
                    |c| json!({ "type": "tool_use", "id": c.id, "name": c.name, "input": c.args }),
                ));
                json!({ "role": "assistant", "content": blocks })
            }
            (role, _) => {
                let role = if *role == Role::Assistant {
                    "assistant"
                } else {
                    "user"
                };
                json!({ "role": role, "content": m.content })
            }
        })
        .collect()
}

#[derive(Default)]
struct StreamState {
    usage: Usage,
    /// Tool call being streamed: id, name and the JSON text received so far.
    tool: Option<(String, String, String)>,
}

impl StreamState {
    fn handle(&mut self, ev: &SseEvent) -> Option<Result<Chunk, AiError>> {
        let v: Value = match serde_json::from_str(&ev.data) {
            Ok(v) => v,
            Err(e) => return Some(Err(AiError::protocol(format!("bad event json: {e}")))),
        };
        match v["type"].as_str()? {
            "message_start" => {
                self.usage.input_tokens = tokens(&v["message"]["usage"]["input_tokens"]);
                None
            }
            "content_block_start" => {
                let block = &v["content_block"];
                if block["type"] == "tool_use" {
                    let field = |k: &str| block[k].as_str().unwrap_or_default().to_owned();
                    self.tool = Some((field("id"), field("name"), String::new()));
                }
                None
            }
            "content_block_delta" => {
                let delta = &v["delta"];
                if let Some(part) = delta["partial_json"].as_str() {
                    if let Some((_, _, json)) = &mut self.tool {
                        json.push_str(part);
                    }
                    return None;
                }
                Some(Ok(Chunk::Text(delta["text"].as_str()?.to_owned())))
            }
            "content_block_stop" => {
                let (id, name, json) = self.tool.take()?;
                let args = if json.trim().is_empty() {
                    Ok(json!({}))
                } else {
                    serde_json::from_str(&json)
                };
                Some(match args {
                    Ok(args) => Ok(Chunk::ToolCall(ToolCall { id, name, args })),
                    Err(e) => Err(AiError::protocol(format!("bad tool arguments: {e}"))),
                })
            }
            "message_delta" => {
                self.usage.output_tokens = tokens(&v["usage"]["output_tokens"]);
                None
            }
            "message_stop" => Some(Ok(Chunk::Done(self.usage))),
            "error" => Some(Err(stream_error(&v["error"]))),
            _ => None,
        }
    }
}

fn tokens(v: &Value) -> u32 {
    v.as_u64().unwrap_or(0) as u32
}

fn stream_error(e: &Value) -> AiError {
    let kind = match e["type"].as_str() {
        Some("authentication_error" | "permission_error") => AiErrorKind::Auth,
        Some("rate_limit_error") => AiErrorKind::RateLimited,
        Some("invalid_request_error") => AiErrorKind::BadRequest,
        Some("overloaded_error" | "api_error") => AiErrorKind::Network,
        _ => AiErrorKind::Other,
    };
    AiError::new(kind, e["message"].as_str().unwrap_or("stream error"))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::sse::SseParser;
    use crate::tools::ToolResult;

    const STREAM: &str = "\
event: message_start
data: {\"type\":\"message_start\",\"message\":{\"usage\":{\"input_tokens\":12,\"output_tokens\":1}}}

event: ping
data: {\"type\":\"ping\"}

event: content_block_delta
data: {\"type\":\"content_block_delta\",\"index\":0,\"delta\":{\"type\":\"text_delta\",\"text\":\"SELECT\"}}

event: content_block_delta
data: {\"type\":\"content_block_delta\",\"index\":0,\"delta\":{\"type\":\"text_delta\",\"text\":\" 1\"}}

event: message_delta
data: {\"type\":\"message_delta\",\"usage\":{\"output_tokens\":5}}

event: message_stop
data: {\"type\":\"message_stop\"}

";

    #[test]
    fn maps_stream_to_chunks() {
        let mut state = StreamState::default();
        let chunks: Vec<_> = SseParser::default()
            .feed(STREAM.as_bytes())
            .iter()
            .filter_map(|e| state.handle(e))
            .collect::<Result<_, _>>()
            .unwrap();
        assert_eq!(
            chunks,
            vec![
                Chunk::Text("SELECT".into()),
                Chunk::Text(" 1".into()),
                Chunk::Done(Usage {
                    input_tokens: 12,
                    output_tokens: 5
                }),
            ]
        );
    }

    const TOOL_STREAM: &str = "\
data: {\"type\":\"content_block_start\",\"index\":1,\"content_block\":{\"type\":\"tool_use\",\"id\":\"tu_1\",\"name\":\"describe_table\"}}

data: {\"type\":\"content_block_delta\",\"index\":1,\"delta\":{\"type\":\"input_json_delta\",\"partial_json\":\"{\\\"tab\"}}

data: {\"type\":\"content_block_delta\",\"index\":1,\"delta\":{\"type\":\"input_json_delta\",\"partial_json\":\"le\\\":\\\"users\\\"}\"}}

data: {\"type\":\"content_block_stop\",\"index\":1}

";

    #[test]
    fn assembles_streamed_tool_call() {
        let mut state = StreamState::default();
        let chunks: Vec<_> = SseParser::default()
            .feed(TOOL_STREAM.as_bytes())
            .iter()
            .filter_map(|e| state.handle(e))
            .collect::<Result<_, _>>()
            .unwrap();
        assert_eq!(
            chunks,
            vec![Chunk::ToolCall(ToolCall {
                id: "tu_1".into(),
                name: "describe_table".into(),
                args: json!({ "table": "users" }),
            })]
        );
    }

    #[test]
    fn wires_tool_turns() {
        let call = ToolCall {
            id: "tu_1".into(),
            name: "list_tables".into(),
            args: json!({}),
        };
        let msgs = vec![
            Message::user("q"),
            Message::assistant_calls("", vec![call.clone()]),
            Message::tool(&call, ToolResult::ok("users")),
        ];
        let w = wire_messages(&msgs);
        assert_eq!(w[1]["content"][0]["type"], "tool_use");
        assert_eq!(w[2]["role"], "user");
        assert_eq!(w[2]["content"][0]["tool_use_id"], "tu_1");
    }

    #[test]
    fn maps_error_event() {
        let ev = SseEvent {
            event: Some("error".into()),
            data: r#"{"type":"error","error":{"type":"overloaded_error","message":"busy"}}"#.into(),
        };
        let err = StreamState::default().handle(&ev).unwrap().unwrap_err();
        assert_eq!(err.kind, AiErrorKind::Network);
        assert_eq!(err.message, "busy");
    }

    #[test]
    fn request_body_shape() {
        let req = ChatRequest::new("claude-x", vec![Message::user("hi")]).with_system("sys");
        let b = body(&req);
        assert_eq!(b["model"], "claude-x");
        assert_eq!(b["stream"], true);
        assert_eq!(b["system"], "sys");
        assert_eq!(b["messages"][0]["role"], "user");
        assert!(b.get("temperature").is_none());
    }
}
