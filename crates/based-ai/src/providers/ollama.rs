use futures::{StreamExt, future};
use reqwest::Client;
use serde_json::{Value, json};

use crate::error::{AiError, AiErrorKind};
use crate::http::{lazy_stream, ndjson_lines};
use crate::provider::{ChunkStream, Provider};
use crate::types::{ChatRequest, Chunk, Usage};

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
                .filter_map(|line| {
                    future::ready(match line {
                        Ok(line) => handle(&line),
                        Err(e) => Some(Err(e)),
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
    messages.extend(r.messages.iter().map(|m| json!(m)));
    let mut options = json!({ "num_predict": r.max_tokens });
    if let Some(t) = r.temperature {
        options["temperature"] = json!(t);
    }
    json!({
        "model": r.model,
        "stream": true,
        "messages": messages,
        "options": options,
    })
}

fn handle(line: &str) -> Option<Result<Chunk, AiError>> {
    let v: Value = match serde_json::from_str(line) {
        Ok(v) => v,
        Err(e) => return Some(Err(AiError::protocol(format!("bad line json: {e}")))),
    };
    if let Some(msg) = v["error"].as_str() {
        return Some(Err(AiError::new(AiErrorKind::BadRequest, msg)));
    }
    if v["done"].as_bool() == Some(true) {
        let count = |k: &str| v[k].as_u64().unwrap_or(0) as u32;
        return Some(Ok(Chunk::Done(Usage {
            input_tokens: count("prompt_eval_count"),
            output_tokens: count("eval_count"),
        })));
    }
    let text = v["message"]["content"].as_str()?;
    (!text.is_empty()).then(|| Ok(Chunk::Text(text.to_owned())))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::types::Message;

    #[test]
    fn maps_lines_to_chunks() {
        let lines = [
            r#"{"message":{"role":"assistant","content":"SELECT"},"done":false}"#,
            r#"{"message":{"role":"assistant","content":" 1"},"done":false}"#,
            r#"{"message":{"content":""},"done":true,"prompt_eval_count":7,"eval_count":3}"#,
        ];
        let chunks: Vec<_> = lines
            .iter()
            .filter_map(|l| handle(l))
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
    fn maps_error_line() {
        let err = handle(r#"{"error":"model not found"}"#)
            .unwrap()
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
