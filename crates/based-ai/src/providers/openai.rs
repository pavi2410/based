use futures::{StreamExt, future};
use reqwest::Client;
use serde_json::{Value, json};

use crate::error::AiError;
use crate::http::{lazy_stream, sse_events};
use crate::provider::{ChunkStream, Provider};
use crate::sse::SseEvent;
use crate::types::{ChatRequest, Chunk, Usage};

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
    let mut messages = Vec::with_capacity(r.messages.len() + 1);
    if let Some(s) = &r.system {
        messages.push(json!({ "role": "system", "content": s }));
    }
    messages.extend(r.messages.iter().map(|m| json!(m)));
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
    v
}

#[derive(Default)]
struct StreamState {
    usage: Usage,
}

impl StreamState {
    fn handle(&mut self, ev: &SseEvent) -> Option<Result<Chunk, AiError>> {
        if ev.data == "[DONE]" {
            return Some(Ok(Chunk::Done(self.usage)));
        }
        let v: Value = match serde_json::from_str(&ev.data) {
            Ok(v) => v,
            Err(e) => return Some(Err(AiError::protocol(format!("bad event json: {e}")))),
        };
        if let Some(e) = v.get("error").filter(|e| !e.is_null()) {
            let msg = e["message"].as_str().unwrap_or("stream error");
            return Some(Err(AiError::protocol(msg)));
        }
        if let Some(u) = v.get("usage").filter(|u| !u.is_null()) {
            self.usage = Usage {
                input_tokens: u["prompt_tokens"].as_u64().unwrap_or(0) as u32,
                output_tokens: u["completion_tokens"].as_u64().unwrap_or(0) as u32,
            };
        }
        let text = v["choices"][0]["delta"]["content"].as_str()?;
        (!text.is_empty()).then(|| Ok(Chunk::Text(text.to_owned())))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::sse::SseParser;
    use crate::types::Message;

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
            .filter_map(|e| state.handle(e))
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
    fn request_body_puts_system_first() {
        let req = ChatRequest::new("gpt-x", vec![Message::user("hi")]).with_system("sys");
        let b = body(&req);
        assert_eq!(b["messages"][0]["role"], "system");
        assert_eq!(b["messages"][1]["content"], "hi");
        assert_eq!(b["stream_options"]["include_usage"], true);
    }
}
