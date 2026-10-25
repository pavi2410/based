//! Tokio -> GPUI bridge for streamed completions.
//!
//! The provider runs on Tokio and pushes chunks into a channel; a GPUI task drains it
//! in batches so a burst of tokens costs one entity update. Dropping [`AiStream`]
//! cancels both sides (the Tokio task aborts when its GPUI handle drops).

use std::sync::Arc;

use based_ai::{AiError, ChatRequest, Chunk, Provider, Usage};
use futures::StreamExt;
use futures::channel::mpsc;
use gpui_kit::{Context, Task};

use super::status::AiStatus;
use crate::db::Tokio;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum StreamEvent {
    Text(String),
    Done(Usage),
    Failed(AiError),
}

impl StreamEvent {
    fn is_terminal(&self) -> bool {
        !matches!(self, Self::Text(_))
    }
}

/// Keep alive while the response is wanted; drop to cancel.
#[must_use = "dropping an AiStream cancels the request"]
pub struct AiStream(#[allow(dead_code)] Task<()>);

/// Starts `request` on `provider` and calls `on_event` on the GPUI thread.
/// Exactly one terminal event (`Done` or `Failed`) is delivered unless cancelled.
pub fn start<T: 'static>(
    provider: Arc<dyn Provider>,
    request: ChatRequest,
    cx: &mut Context<T>,
    mut on_event: impl FnMut(&mut T, StreamEvent, &mut Context<T>) + 'static,
) -> AiStream {
    let status = AiStatus::get(cx);
    let guard = status.begin();
    let (tx, mut rx) = mpsc::unbounded();
    let pump = Tokio::spawn(&*cx, async move {
        let mut chunks = provider.stream(request);
        while let Some(item) = chunks.next().await {
            if tx.unbounded_send(item).is_err() {
                break;
            }
        }
    });
    AiStream(cx.spawn(async move |this, cx| {
        let _pump = pump;
        let _guard = guard;
        let mut finished = false;
        while !finished {
            let mut batch = Vec::new();
            match rx.next().await {
                Some(first) => batch.push(first),
                None => break,
            }
            while let Ok(more) = rx.try_recv() {
                batch.push(more);
            }
            let events = merge(batch);
            finished = events.last().is_some_and(StreamEvent::is_terminal);
            for event in &events {
                if let StreamEvent::Done(usage) = event {
                    status.add_usage(*usage);
                }
            }
            let delivered = this.update(cx, |t, cx| {
                for event in events {
                    on_event(t, event, cx);
                }
            });
            if delivered.is_err() {
                return;
            }
            if finished {
                cx.update(|app| app.refresh_windows());
            }
        }
        if !finished {
            let ended = StreamEvent::Failed(AiError::protocol("response ended unexpectedly"));
            let _ = this.update(cx, |t, cx| on_event(t, ended, cx));
            cx.update(|app| app.refresh_windows());
        }
    }))
}

/// Maps chunks to events, joins adjacent text, and drops anything after a terminal event.
fn merge(batch: Vec<Result<Chunk, AiError>>) -> Vec<StreamEvent> {
    let mut out: Vec<StreamEvent> = Vec::new();
    for item in batch {
        let event = match item {
            Ok(Chunk::Text(t)) => StreamEvent::Text(t),
            Ok(Chunk::Done(u)) => StreamEvent::Done(u),
            Err(e) => StreamEvent::Failed(e),
        };
        match (out.last_mut(), event) {
            (Some(last), _) if last.is_terminal() => break,
            (Some(StreamEvent::Text(prev)), StreamEvent::Text(next)) => prev.push_str(&next),
            (_, event) => out.push(event),
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use based_ai::AiErrorKind;

    use super::*;

    fn text(s: &str) -> Result<Chunk, AiError> {
        Ok(Chunk::Text(s.into()))
    }

    #[test]
    fn joins_adjacent_text() {
        let got = merge(vec![text("SEL"), text("ECT"), text(" 1")]);
        assert_eq!(got, vec![StreamEvent::Text("SELECT 1".into())]);
    }

    #[test]
    fn stops_after_terminal_event() {
        let err = AiError::new(AiErrorKind::Network, "down");
        let got = merge(vec![text("a"), Err(err.clone()), text("late")]);
        assert_eq!(
            got,
            vec![StreamEvent::Text("a".into()), StreamEvent::Failed(err)]
        );

        let done = Chunk::Done(Usage::default());
        let got = merge(vec![Ok(done), text("late")]);
        assert_eq!(got, vec![StreamEvent::Done(Usage::default())]);
    }
}
