use futures::{StreamExt, stream};
use reqwest::Client;
use serde_json::{Value, json};

use crate::error::{AiError, AiErrorKind};
use crate::http::{lazy_stream, ndjson_lines};
use crate::provider::{ChunkStream, Provider};
use crate::tools::{ToolCall, ToolSpec};
use crate::types::{ChatRequest, Chunk, Message, Role, Usage};

const DEFAULT_URL: &str = "http://localhost:11434";

/// Local Ollama server (`/api/chat`, newline-delimited JSON).
pub struct Ollama {
    client: Client,
    base_url: String,
}

impl Default for Ollama {
    fn default() -> Self {
        Self {
            client: Client::new(),
            base_url: DEFAULT_URL.into(),
        }
    }
}

impl Ollama {
    pub fn with_base_url(mut self, url: impl Into<String>) -> Self {
        self.base_url = url.into().trim_end_matches('/').to_owned();
        self
    }
}

impl Provider for Ollama {
    fn name(&self) -> &'static str {
        "ollama"
    }

    fn stream(&self, request: ChatRequest) -> ChunkStream {
        let req = self
            .client
            .post(format!("{}/api/chat", self.base_url))
            .json(&body(&request));
        lazy_stream(req, |resp| {
            ndjson_lines(resp)
                .flat_map(|line| {
                    stream::iter(match line {
                        Ok(line) => handle(&line),
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
    let mut options = json!({ "num_predict": r.max_tokens });
    if let Some(t) = r.temperature {
        options["temperature"] = json!(t);
    }
    let mut v = json!({
        "model": r.model,
        "stream": true,
        "messages": messages,
        "options": options,
    });
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
            json!({ "role": "tool", "tool_name": o.name, "content": m.content })
        }
        (Role::Assistant, _) if !m.calls.is_empty() => json!({
            "role": "assistant",
            "content": m.content,
            "tool_calls": m.calls.iter().map(|c| json!({
                "function": { "name": c.name, "arguments": c.args },
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

fn handle(line: &str) -> Vec<Result<Chunk, AiError>> {
    let v: Value = match serde_json::from_str(line) {
        Ok(v) => v,
        Err(e) => return vec![Err(AiError::protocol(format!("bad line json: {e}")))],
    };
    if let Some(msg) = v["error"].as_str() {
        return vec![Err(AiError::new(AiErrorKind::BadRequest, msg))];
    }
    let mut out = Vec::new();
    if let Some(text) = v["message"]["content"].as_str().filter(|t| !t.is_empty()) {
        out.push(Ok(Chunk::Text(text.to_owned())));
    }
    // Ollama sends whole calls with object arguments and no ids.
    for (i, call) in v["message"]["tool_calls"]
        .as_array()
        .into_iter()
        .flatten()
        .enumerate()
    {
        let f = &call["function"];
        out.push(Ok(Chunk::ToolCall(ToolCall {
            id: format!("call_{i}"),
            name: f["name"].as_str().unwrap_or_default().to_owned(),
            args: f["arguments"].clone(),
        })));
    }
    if v["done"].as_bool() == Some(true) {
        let count = |k: &str| v[k].as_u64().unwrap_or(0) as u32;
        out.push(Ok(Chunk::Done(Usage {
            input_tokens: count("prompt_eval_count"),
            output_tokens: count("eval_count"),
        })));
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn maps_lines_to_chunks() {
        let lines = [
            r#"{"message":{"role":"assistant","content":"SELECT"},"done":false}"#,
            r#"{"message":{"role":"assistant","content":" 1"},"done":false}"#,
            r#"{"message":{"content":""},"done":true,"prompt_eval_count":7,"eval_count":3}"#,
        ];
        let chunks: Vec<_> = lines
            .iter()
            .flat_map(|l| handle(l))
            .collect::<Result<_, _>>()
            .unwrap();
        assert_eq!(
            chunks,
            vec![
                Chunk::Text("SELECT".into()),
                Chunk::Text(" 1".into()),
                Chunk::Done(Usage {
                    input_tokens: 7,
                    output_tokens: 3
                }),
            ]
        );
    }

    #[test]
    fn maps_tool_call_line() {
        let line = r#"{"message":{"role":"assistant","content":"","tool_calls":[{"function":{"name":"describe_table","arguments":{"table":"users"}}}]},"done":false}"#;
        assert_eq!(
            handle(line),
            vec![Ok(Chunk::ToolCall(ToolCall {
                id: "call_0".into(),
                name: "describe_table".into(),
                args: json!({ "table": "users" }),
            }))]
        );
    }

    #[test]
    fn maps_error_line() {
        let err = handle(r#"{"error":"model not found"}"#)
            .remove(0)
            .unwrap_err();
        assert_eq!(err.kind, AiErrorKind::BadRequest);
    }

    #[test]
    fn request_body_shape() {
        let mut req = ChatRequest::new("llama3", vec![Message::user("hi")]).with_system("sys");
        req.temperature = Some(0.2);
        let b = body(&req);
        assert_eq!(b["messages"][0]["role"], "system");
        assert_eq!(b["options"]["num_predict"], 2048);
        assert!(b["options"]["temperature"].is_number());
    }
}
