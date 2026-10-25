use futures::{StreamExt, future};
use reqwest::Client;
use serde_json::{Value, json};

use crate::error::AiError;
use crate::http::{lazy_stream, sse_events};
use crate::provider::{ChunkStream, Provider};
use crate::sse::SseEvent;
use crate::tools::{ToolCall, ToolSpec};
use crate::types::{ChatRequest, Chunk, Message, Role, Usage};

const DEFAULT_URL: &str = "https://api.openai.com/v1";

/// OpenAI Responses API (`/v1/responses`), which supports function tools on reasoning models.
/// Servers that only speak chat completions are not supported.
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
            .post(format!("{}/responses", self.base_url))
            .bearer_auth(&self.api_key)
            .json(&body(&request));
        lazy_stream(req, |resp| {
            sse_events(resp)
                .filter_map(|ev| {
                    future::ready(match ev {
                        Ok(ev) => handle(&ev),
                        Err(e) => Some(Err(e)),
                    })
                })
                .boxed()
        })
    }
}

fn body(r: &ChatRequest) -> Value {
    // No `temperature`: reasoning models reject it.
    let mut v = json!({
        "model": r.model,
        "input": r.messages.iter().flat_map(wire_items).collect::<Vec<_>>(),
        "max_output_tokens": r.max_tokens,
        "stream": true,
        "store": false,
    });
    if let Some(s) = &r.system {
        v["instructions"] = json!(s);
    }
    if !r.tools.is_empty() {
        v["tools"] = r.tools.iter().map(wire_tool).collect();
    }
    v
}

fn wire_tool(t: &ToolSpec) -> Value {
    json!({
        "type": "function",
        "name": t.name,
        "description": t.description,
        "parameters": t.schema,
    })
}

/// One message becomes one or more input items.
fn wire_items(m: &Message) -> Vec<Value> {
    match (&m.role, &m.outcome) {
        (Role::Tool, Some(o)) => {
            vec![json!({ "type": "function_call_output", "call_id": o.id, "output": m.content })]
        }
        (Role::Assistant, _) => {
            let mut items = Vec::new();
            if !m.content.is_empty() {
                items.push(json!({ "role": "assistant", "content": m.content }));
            }
            items.extend(m.calls.iter().map(|c| {
                json!({
                    "type": "function_call",
                    "call_id": c.id,
                    "name": c.name,
                    "arguments": c.args.to_string(),
                })
            }));
            items
        }
        _ => vec![json!({ "role": "user", "content": m.content })],
    }
}

fn handle(ev: &SseEvent) -> Option<Result<Chunk, AiError>> {
    let v: Value = match serde_json::from_str(&ev.data) {
        Ok(v) => v,
        Err(e) => return Some(Err(AiError::protocol(format!("bad event json: {e}")))),
    };
    match v["type"].as_str()? {
        "response.output_text.delta" => {
            let text = v["delta"].as_str().filter(|t| !t.is_empty())?;
            Some(Ok(Chunk::Text(text.to_owned())))
        }
        "response.output_item.done" => {
            let item = &v["item"];
            if item["type"] != "function_call" {
                return None;
            }
            let args = match item["arguments"].as_str().map(str::trim) {
                None | Some("") => Ok(json!({})),
                Some(s) => serde_json::from_str(s),
            };
            let field = |k: &str| item[k].as_str().unwrap_or_default().to_owned();
            Some(match args {
                Ok(args) => Ok(Chunk::ToolCall(ToolCall {
                    id: field("call_id"),
                    name: field("name"),
                    args,
                })),
                Err(e) => Err(AiError::protocol(format!("bad tool arguments: {e}"))),
            })
        }
        "response.completed" | "response.incomplete" => {
            let u = &v["response"]["usage"];
            let count = |k: &str| u[k].as_u64().unwrap_or(0) as u32;
            Some(Ok(Chunk::Done(Usage {
                input_tokens: count("input_tokens"),
                output_tokens: count("output_tokens"),
            })))
        }
        "response.failed" => {
            let msg = v["response"]["error"]["message"].as_str();
            Some(Err(AiError::protocol(msg.unwrap_or("response failed"))))
        }
        "error" => {
            let msg = v["message"].as_str().or(v["error"]["message"].as_str());
            Some(Err(AiError::protocol(msg.unwrap_or("stream error"))))
        }
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::sse::SseParser;
    use crate::tools::ToolResult;

    const STREAM: &str = "\
event: response.created
data: {\"type\":\"response.created\",\"response\":{}}

event: response.output_text.delta
data: {\"type\":\"response.output_text.delta\",\"delta\":\"SELECT\"}

event: response.output_text.delta
data: {\"type\":\"response.output_text.delta\",\"delta\":\" 1\"}

event: response.completed
data: {\"type\":\"response.completed\",\"response\":{\"usage\":{\"input_tokens\":9,\"output_tokens\":4}}}

";

    fn chunks(stream: &str) -> Vec<Chunk> {
        SseParser::default()
            .feed(stream.as_bytes())
            .iter()
            .filter_map(handle)
            .collect::<Result<_, _>>()
            .unwrap()
    }

    #[test]
    fn maps_stream_to_chunks() {
        assert_eq!(
            chunks(STREAM),
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
    fn emits_finished_function_calls() {
        let stream = "\
data: {\"type\":\"response.output_item.added\",\"item\":{\"type\":\"function_call\",\"call_id\":\"c1\",\"name\":\"describe_table\",\"arguments\":\"\"}}

data: {\"type\":\"response.function_call_arguments.delta\",\"delta\":\"{\\\"table\\\"\"}

data: {\"type\":\"response.output_item.done\",\"item\":{\"type\":\"function_call\",\"call_id\":\"c1\",\"name\":\"describe_table\",\"arguments\":\"{\\\"table\\\":\\\"users\\\"}\"}}

data: {\"type\":\"response.output_item.done\",\"item\":{\"type\":\"message\"}}

";
        assert_eq!(
            chunks(stream),
            vec![Chunk::ToolCall(ToolCall {
                id: "c1".into(),
                name: "describe_table".into(),
                args: json!({ "table": "users" }),
            })]
        );
    }

    #[test]
    fn maps_failures() {
        let ev = SseEvent {
            event: None,
            data: r#"{"type":"response.failed","response":{"error":{"message":"boom"}}}"#.into(),
        };
        assert_eq!(handle(&ev).unwrap().unwrap_err().message, "boom");
    }

    #[test]
    fn wires_tool_turns_as_items() {
        let call = ToolCall {
            id: "c1".into(),
            name: "list_tables".into(),
            args: json!({}),
        };
        let a = wire_items(&Message::assistant_calls("", vec![call.clone()]));
        assert_eq!(a.len(), 1);
        assert_eq!(a[0]["type"], "function_call");
        assert_eq!(a[0]["call_id"], "c1");
        let t = wire_items(&Message::tool(&call, ToolResult::ok("users")));
        assert_eq!(t[0]["type"], "function_call_output");
        assert_eq!(t[0]["output"], "users");
    }

    #[test]
    fn request_body_shape() {
        let mut req = ChatRequest::new("gpt-x", vec![Message::user("hi")]).with_system("sys");
        req.temperature = Some(0.5);
        let b = body(&req);
        assert_eq!(b["instructions"], "sys");
        assert_eq!(b["input"][0]["content"], "hi");
        assert_eq!(b["stream"], true);
        assert!(b.get("temperature").is_none());
        assert!(b.get("tools").is_none());
    }
}
