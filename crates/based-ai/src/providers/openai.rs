use futures::{StreamExt, stream};
use reqwest::Client;
use serde_json::{Value, json};

use crate::error::AiError;
use crate::http::{lazy_stream, sse_events};
use crate::provider::{ChunkStream, Provider};
use crate::sse::SseEvent;
use crate::tools::{ToolCall, ToolSpec};
use crate::types::{ChatRequest, Chunk, Message, Role, Usage};

const DEFAULT_URL: &str = "https://api.openai.com/v1";

/// OpenAI chat completions. Also works with OpenAI-compatible servers via `with_base_url`.
pub struct OpenAi {
    client: Client,
    api_key: String,
    base_url: String,
}

impl OpenAi {
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

impl Provider for OpenAi {
    fn name(&self) -> &'static str {
        "openai"
    }

    fn stream(&self, request: ChatRequest) -> ChunkStream {
        let req = self
            .client
            .post(format!("{}/chat/completions", self.base_url))
            .bearer_auth(&self.api_key)
            .json(&body(&request));
        lazy_stream(req, |resp| {
            let mut state = StreamState::default();
            sse_events(resp)
                .flat_map(move |ev| {
                    stream::iter(match ev {
                        Ok(ev) => state.handle(&ev),
                        Err(e) => vec![Err(e)],
                    })
                })
                .boxed()
        })
    }
}

fn body(r: &ChatRequest) -> Value {
    let mut messages = Vec::with_capacity(r.messages.len() + 1);
    if let Some(s) = &r.system {
        messages.push(json!({ "role": "system", "content": s }));
    }
    messages.extend(r.messages.iter().map(wire_message));
    let mut v = json!({
        "model": r.model,
        "max_completion_tokens": r.max_tokens,
        "stream": true,
        "stream_options": { "include_usage": true },
        "messages": messages,
    });
    if let Some(t) = r.temperature {
        v["temperature"] = json!(t);
    }
    if !r.tools.is_empty() {
        v["tools"] = r.tools.iter().map(wire_tool).collect();
    }
    v
}

fn wire_tool(t: &ToolSpec) -> Value {
    json!({
        "type": "function",
        "function": { "name": t.name, "description": t.description, "parameters": t.schema },
    })
}

fn wire_message(m: &Message) -> Value {
    match (&m.role, &m.outcome) {
        (Role::Tool, Some(o)) => {
            json!({ "role": "tool", "tool_call_id": o.id, "content": m.content })
        }
        (Role::Assistant, _) if !m.calls.is_empty() => json!({
            "role": "assistant",
            "content": (!m.content.is_empty()).then_some(&m.content),
            "tool_calls": m.calls.iter().map(|c| json!({
                "id": c.id,
                "type": "function",
                "function": { "name": c.name, "arguments": c.args.to_string() },
            })).collect::<Vec<_>>(),
        }),
        (role, _) => {
            let role = if *role == Role::Assistant {
                "assistant"
            } else {
                "user"
            };
            json!({ "role": role, "content": m.content })
        }
    }
}

/// A tool call whose arguments are still arriving.
#[derive(Default)]
struct PartialCall {
    id: String,
    name: String,
    args: String,
}

#[derive(Default)]
struct StreamState {
    usage: Usage,
    calls: Vec<PartialCall>,
}

impl StreamState {
    fn handle(&mut self, ev: &SseEvent) -> Vec<Result<Chunk, AiError>> {
        if ev.data == "[DONE]" {
            let mut out = self.flush();
            out.push(Ok(Chunk::Done(self.usage)));
            return out;
        }
        let v: Value = match serde_json::from_str(&ev.data) {
            Ok(v) => v,
            Err(e) => return vec![Err(AiError::protocol(format!("bad event json: {e}")))],
        };
        if let Some(e) = v.get("error").filter(|e| !e.is_null()) {
            let msg = e["message"].as_str().unwrap_or("stream error");
            return vec![Err(AiError::protocol(msg))];
        }
        if let Some(u) = v.get("usage").filter(|u| !u.is_null()) {
            self.usage = Usage {
                input_tokens: u["prompt_tokens"].as_u64().unwrap_or(0) as u32,
                output_tokens: u["completion_tokens"].as_u64().unwrap_or(0) as u32,
            };
        }
        let choice = &v["choices"][0];
        let delta = &choice["delta"];
        if let Some(parts) = delta["tool_calls"].as_array() {
            for part in parts {
                self.absorb(part);
            }
        }
        let mut out = Vec::new();
        if let Some(text) = delta["content"].as_str().filter(|t| !t.is_empty()) {
            out.push(Ok(Chunk::Text(text.to_owned())));
        }
        if !choice["finish_reason"].is_null() {
            out.extend(self.flush());
        }
        out
    }

    fn absorb(&mut self, part: &Value) {
        let ix = part["index"].as_u64().unwrap_or(0) as usize;
        if self.calls.len() <= ix {
            self.calls.resize_with(ix + 1, PartialCall::default);
        }
        let call = &mut self.calls[ix];
        if let Some(id) = part["id"].as_str() {
            call.id = id.to_owned();
        }
        let f = &part["function"];
        if let Some(name) = f["name"].as_str() {
            call.name.push_str(name);
        }
        if let Some(args) = f["arguments"].as_str() {
            call.args.push_str(args);
        }
    }

    fn flush(&mut self) -> Vec<Result<Chunk, AiError>> {
        self.calls
            .drain(..)
            .enumerate()
            .map(|(i, c)| {
                let args = if c.args.trim().is_empty() {
                    Ok(json!({}))
                } else {
                    serde_json::from_str(&c.args)
                };
                let id = if c.id.is_empty() {
                    format!("call_{i}")
                } else {
                    c.id
                };
                args.map(|args| {
                    Chunk::ToolCall(ToolCall {
                        id,
                        name: c.name,
                        args,
                    })
                })
                .map_err(|e| AiError::protocol(format!("bad tool arguments: {e}")))
            })
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::sse::SseParser;
    use crate::tools::ToolResult;

    const STREAM: &str = "\
data: {\"choices\":[{\"delta\":{\"role\":\"assistant\",\"content\":\"\"}}],\"usage\":null}

data: {\"choices\":[{\"delta\":{\"content\":\"SELECT\"}}],\"usage\":null}

data: {\"choices\":[{\"delta\":{\"content\":\" 1\"}}],\"usage\":null}

data: {\"choices\":[{\"delta\":{},\"finish_reason\":\"stop\"}],\"usage\":null}

data: {\"choices\":[],\"usage\":{\"prompt_tokens\":9,\"completion_tokens\":4}}

data: [DONE]

";

    #[test]
    fn maps_stream_to_chunks() {
        let mut state = StreamState::default();
        let chunks: Vec<_> = SseParser::default()
            .feed(STREAM.as_bytes())
            .iter()
            .flat_map(|e| state.handle(e))
            .collect::<Result<_, _>>()
            .unwrap();
        assert_eq!(
            chunks,
            vec![
                Chunk::Text("SELECT".into()),
                Chunk::Text(" 1".into()),
                Chunk::Done(Usage {
                    input_tokens: 9,
                    output_tokens: 4
                }),
            ]
        );
    }

    #[test]
    fn assembles_tool_call_across_deltas() {
        let events = [
            r#"{"choices":[{"delta":{"tool_calls":[{"index":0,"id":"c1","function":{"name":"describe_table","arguments":""}}]}}]}"#,
            r#"{"choices":[{"delta":{"tool_calls":[{"index":0,"function":{"arguments":"{\"table\":"}}]}}]}"#,
            r#"{"choices":[{"delta":{"tool_calls":[{"index":0,"function":{"arguments":"\"users\"}"}}]}}]}"#,
            r#"{"choices":[{"delta":{},"finish_reason":"tool_calls"}]}"#,
        ];
        let mut state = StreamState::default();
        let chunks: Vec<_> = events
            .iter()
            .flat_map(|d| {
                state.handle(&SseEvent {
                    event: None,
                    data: (*d).into(),
                })
            })
            .collect::<Result<_, _>>()
            .unwrap();
        assert_eq!(
            chunks,
            vec![Chunk::ToolCall(ToolCall {
                id: "c1".into(),
                name: "describe_table".into(),
                args: json!({ "table": "users" }),
            })]
        );
    }

    #[test]
    fn wires_tool_turns() {
        let call = ToolCall {
            id: "c1".into(),
            name: "list_tables".into(),
            args: json!({}),
        };
        let a = wire_message(&Message::assistant_calls("", vec![call.clone()]));
        assert!(a["content"].is_null());
        assert_eq!(a["tool_calls"][0]["function"]["name"], "list_tables");
        let t = wire_message(&Message::tool(&call, ToolResult::ok("users")));
        assert_eq!(
            (t["role"].as_str(), t["tool_call_id"].as_str()),
            (Some("tool"), Some("c1"))
        );
    }

    #[test]
    fn request_body_puts_system_first() {
        let req = ChatRequest::new("gpt-x", vec![Message::user("hi")]).with_system("sys");
        let b = body(&req);
        assert_eq!(b["messages"][0]["role"], "system");
        assert_eq!(b["messages"][1]["content"], "hi");
        assert_eq!(b["stream_options"]["include_usage"], true);
    }
}
