//! Starts a model request from app state: reads provider prefs, fetches the API key off the
//! UI thread, builds the provider and streams the reply into an entity.

use based_ai::{AiError, AiErrorKind, ChatRequest};
use based_storage::SecretStore;
use gpui_kit::{Context, Task};

use super::stream::{AiStream, StreamEvent, start};
use crate::app::prefs;

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
        let key = cx
            .background_executor()
            .spawn(async move {
                if kind.needs_key() {
                    SecretStore::new().get_ai_key(kind.as_str())
                } else {
                    Ok(None)
                }
            })
            .await;
        let _ = this.update(cx, |this, cx| {
            let provider = key
                .map_err(|e| AiError::new(AiErrorKind::Other, format!("{e:#}")))
                .and_then(|key| kind.build(key.as_deref(), base_url.as_deref()));
            match provider {
                Ok(provider) => attach(this, start(provider, request, cx, on_event)),
                Err(e) => on_event(this, StreamEvent::Failed(e), cx),
            }
        });
    })
}

/// User-facing text for a failed request.
pub fn error_text(e: &AiError) -> String {
    match e.kind {
        AiErrorKind::Auth => format!("{} Set the key in Settings > AI.", e.message),
        _ => e.message.clone(),
    }
}
