//! What every editor panel needs to host the prompt bar: open state, requests, and the
//! "AI wrote this" tracking behind write confirmation. Panels keep only what is theirs
//! (reading and replacing editor text, schema context, how they confirm a run).

use based_ai::tools::check;
use based_core::EngineKind;
use based_project::AiSettings;
use gpui_kit::{App, AppContext as _, Context, Entity, Window};

use super::prompt::{AiPrompt, AiPromptEvent};
use crate::connection::ConnectionId;
use crate::workspace::WorkspaceRef;

pub const OFF_NOTICE: &str = "AI is off for this connection. Enable it in the connection settings.";

/// The connection's `[ai]` settings, if it is in the registry.
pub fn connection_ai(conn: &ConnectionId, cx: &App) -> Option<AiSettings> {
    let ws = cx.try_global::<WorkspaceRef>()?;
    let registry = ws.0.read(cx).registry().read(cx);
    registry.get(conn, cx)?.read(cx).ai
}

pub struct AiHost {
    prompt: Entity<AiPrompt>,
    open: bool,
    /// Text last applied from a proposal; running it unchanged may need confirmation.
    applied: Option<String>,
}

impl AiHost {
    /// `on_event` receives the bar's events (`Submit`, `Apply`, `Close`) in the panel.
    pub fn new<P: 'static>(
        window: &mut Window,
        cx: &mut Context<P>,
        on_event: impl Fn(&mut P, &AiPromptEvent, &mut Window, &mut Context<P>) + 'static,
    ) -> Self {
        let prompt = cx.new(|cx| AiPrompt::new(window, cx));
        cx.subscribe_in(&prompt, window, move |this, _, event, window, cx| {
            on_event(this, event, window, cx);
        })
        .detach();
        Self {
            prompt,
            open: false,
            applied: None,
        }
    }

    pub fn is_open(&self) -> bool {
        self.open
    }

    /// The bar, when it should be shown.
    pub fn bar(&self) -> Option<Entity<AiPrompt>> {
        self.open.then(|| self.prompt.clone())
    }

    pub fn prompt(&self) -> &Entity<AiPrompt> {
        &self.prompt
    }

    /// Opens or closes the bar. `off` shows the "AI is off" notice instead of the input.
    pub fn toggle<P: 'static>(&mut self, off: bool, window: &mut Window, cx: &mut Context<P>) {
        if self.open {
            return self.close(cx);
        }
        self.prompt
            .update(cx, |p, cx| p.set_notice(off.then(|| OFF_NOTICE.into()), cx));
        self.open = true;
        self.prompt.update(cx, |p, cx| p.focus(window, cx));
        cx.notify();
    }

    /// Cancels any request and hides the bar. The panel should refocus its editor.
    pub fn close<P: 'static>(&mut self, cx: &mut Context<P>) {
        self.prompt.update(cx, |p, cx| p.reset(cx));
        self.open = false;
        cx.notify();
    }

    pub fn ask<P: 'static>(
        &self,
        engine: EngineKind,
        schema: String,
        current: String,
        instruction: String,
        cx: &mut Context<P>,
    ) {
        self.prompt.update(cx, |p, cx| {
            p.start(engine, schema, current, instruction, cx)
        });
    }

    pub fn note_applied(&mut self, text: &str) {
        self.applied = Some(text.to_owned());
    }

    /// AI-written text that may write must be confirmed before its first run. Text the user
    /// edited after applying is theirs and runs normally.
    pub fn needs_confirm(&self, engine: EngineKind, current: &str) -> bool {
        needs_confirm(engine, self.applied.as_deref(), current)
    }

    pub fn confirmed(&mut self) {
        self.applied = None;
    }
}

fn needs_confirm(engine: EngineKind, applied: Option<&str>, current: &str) -> bool {
    applied.is_some_and(|a| a.trim() == current.trim()) && check(engine, current).is_err()
}

#[cfg(test)]
mod tests {
    use super::*;

    const PG: EngineKind = EngineKind::Postgres;

    #[test]
    fn unchanged_ai_write_needs_confirmation() {
        assert!(needs_confirm(PG, Some("DELETE FROM t"), "DELETE FROM t\n"));
        assert!(needs_confirm(PG, Some("DROP TABLE t"), "DROP TABLE t"));
    }

    #[test]
    fn ai_reads_run_freely() {
        assert!(!needs_confirm(PG, Some("SELECT 1"), "SELECT 1"));
    }

    #[test]
    fn edited_or_manual_text_is_not_gated() {
        assert!(!needs_confirm(
            PG,
            Some("DELETE FROM t"),
            "DELETE FROM t WHERE id = 1"
        ));
        assert!(!needs_confirm(PG, None, "DELETE FROM t"));
    }

    #[test]
    fn redis_writes_are_gated() {
        assert!(needs_confirm(EngineKind::Redis, Some("DEL k"), "DEL k"));
        assert!(!needs_confirm(EngineKind::Redis, Some("GET k"), "GET k"));
    }

    #[test]
    fn mongo_out_stage_is_gated() {
        let p = r#"[{"$match":{}},{"$out":"x"}]"#;
        assert!(needs_confirm(EngineKind::MongoDB, Some(p), p));
    }
}
