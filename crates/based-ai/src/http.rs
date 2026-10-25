//! Shared HTTP plumbing for streaming providers.

use std::collections::VecDeque;
use std::future::ready;

use futures::stream::{self, BoxStream};
use futures::{FutureExt, StreamExt};
use reqwest::{RequestBuilder, Response};

use crate::error::{AiError, AiErrorKind};
use crate::provider::ChunkStream;
use crate::sse::{SseEvent, SseParser};

fn net_err(e: reqwest::Error) -> AiError {
    AiError::new(AiErrorKind::Network, e.to_string())
}

/// Sends the request and maps non-2xx responses to `AiError`.
pub async fn send(req: RequestBuilder) -> Result<Response, AiError> {
    let resp = req.send().await.map_err(net_err)?;
    let status = resp.status();
    if status.is_success() {
        return Ok(resp);
    }
    let body = resp.text().await.unwrap_or_default();
    Err(AiError::from_status(status.as_u16(), body))
}

/// Turns a response body into a stream of SSE events.
pub fn sse_events(resp: Response) -> BoxStream<'static, Result<SseEvent, AiError>> {
    let state = (
        resp.bytes_stream().boxed(),
        SseParser::default(),
        VecDeque::new(),
    );
    stream::unfold(state, |(mut body, mut parser, mut queue)| async move {
        loop {
            if let Some(ev) = queue.pop_front() {
                return Some((Ok(ev), (body, parser, queue)));
            }
            match body.next().await? {
                Ok(bytes) => queue.extend(parser.feed(&bytes)),
                Err(e) => return Some((Err(net_err(e)), (body, parser, queue))),
            }
        }
    })
    .boxed()
}

/// Sends `req` lazily and flattens the response into a chunk stream via `map`.
pub fn lazy_stream(
    req: RequestBuilder,
    map: impl FnOnce(Response) -> ChunkStream + Send + 'static,
) -> ChunkStream {
    send(req)
        .map(move |r| match r {
            Ok(resp) => map(resp),
            Err(e) => stream::once(ready(Err(e))).boxed(),
        })
        .into_stream()
        .flatten()
        .boxed()
}

/// Turns a response body into a stream of non-empty lines (NDJSON).
pub fn ndjson_lines(resp: Response) -> BoxStream<'static, Result<String, AiError>> {
    let state = (resp.bytes_stream().boxed(), Vec::<u8>::new());
    stream::unfold(state, |(mut body, mut buf)| async move {
        loop {
            if let Some(pos) = buf.iter().position(|&b| b == b'\n') {
                let raw: Vec<u8> = buf.drain(..=pos).collect();
                let line = String::from_utf8_lossy(&raw).trim().to_owned();
                if line.is_empty() {
                    continue;
                }
                return Some((Ok(line), (body, buf)));
            }
            match body.next().await {
                Some(Ok(bytes)) => buf.extend_from_slice(&bytes),
                Some(Err(e)) => return Some((Err(net_err(e)), (body, buf))),
                None => {
                    let line = String::from_utf8_lossy(&buf).trim().to_owned();
                    buf.clear();
                    return (!line.is_empty()).then_some((Ok(line), (body, buf)));
                }
            }
        }
    })
    .boxed()
}
