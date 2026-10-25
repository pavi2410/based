//! Starts model requests from app state: reads provider prefs, fetches the API key off the
//! UI thread, builds the provider and either streams the reply into an entity ([`launch`])
//! or returns it whole ([`complete`]).

use std::sync::Arc;

use based_ai::{AiError, AiErrorKind, ChatRequest, Chunk, Provider};
use based_storage::SecretStore;
use futures::StreamExt;
use gpui_kit::{App, AsyncApp, Context, Task};

use super::stream::{AiStream, StreamEvent, start};
use crate::app::prefs;
use crate::db;

/// Builds the configured provider. The keychain read happens off the UI thread.
async fn provider(
    cx: &AsyncApp,
    kind: based_ai::ProviderKind,
    base_url: Option<String>,
) -> Result<Arc<dyn Provider>, AiError> {
    let key = cx
        .background_executor()
        .spawn(async move {
            if kind.needs_key() {
                SecretStore::new().get_ai_key(kind.as_str())
            } else {
                Ok(None)
            }
        })
        .await
        .map_err(|e| AiError::new(AiErrorKind::Other, format!("{e:#}")))?;
    kind.build(key.as_deref(), base_url.as_deref())
}

/// Runs `build(model)` on the configured provider. The stream is handed to `attach` (keep it
/// alive to keep the request running) and events go to `on_event`; setup failures arrive there
/// as `StreamEvent::Failed`. Dropping the returned task cancels a request still in setup.
pub fn launch<T: 'static>(
    cx: &mut Context<T>,
    build: impl FnOnce(&str) -> ChatRequest,
    attach: fn(&mut T, AiStream),
    on_event: fn(&mut T, StreamEvent, &mut Context<T>),
) -> Task<()> {
    let prefs = prefs::ai_prefs(cx);
    let (kind, base_url) = (prefs.provider(), prefs.base_url().map(str::to_owned));
    let request = build(prefs.model());
    cx.spawn(async move |this, cx| {
        let provider = provider(cx, kind, base_url).await;
        let _ = this.update(cx, |this, cx| match provider {
            Ok(provider) => attach(this, start(provider, request, cx, on_event)),
            Err(e) => on_event(this, StreamEvent::Failed(e), cx),
        });
    })
}

/// Runs `build(model)` to completion and returns the whole reply text. Dropping the task
/// cancels the request.
pub fn complete(
    cx: &App,
    build: impl FnOnce(&str) -> ChatRequest,
) -> Task<Result<String, AiError>> {
    let prefs = prefs::ai_prefs(cx);
    let (kind, base_url) = (prefs.provider(), prefs.base_url().map(str::to_owned));
    let request = build(prefs.model());
    cx.spawn(async move |cx| {
        let provider = provider(cx, kind, base_url).await?;
        let reply = db::run_infallible(cx, async move {
            let mut chunks = provider.stream(request);
            let mut text = String::new();
            while let Some(chunk) = chunks.next().await {
                match chunk? {
                    Chunk::Text(t) => text.push_str(&t),
                    Chunk::Done(_) => break,
                    Chunk::ToolCall(_) => {}
                }
            }
            Ok::<_, AiError>(text)
        })
        .await;
        reply.map_err(|e| AiError::new(AiErrorKind::Other, format!("{e:#}")))?
    })
}

/// User-facing text for a failed request.
pub fn error_text(e: &AiError) -> String {
    match e.kind {
        AiErrorKind::Auth => format!("{} Set the key in Settings > AI.", e.message),
        _ => e.message.clone(),
    }
}
