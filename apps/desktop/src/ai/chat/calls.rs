//! Tool calls in the transcript: one status line each, expanding to arguments and output.

use based_ai::tools::ToolCall;
use based_ai::{Message, Role};
use gpui_kit::component::{ActiveTheme, Icon, IconName, Sizable as _, h_flex, v_flex};
use gpui_kit::{
    AnyElement, App, Context, Entity, InteractiveElement as _, IntoElement, ParentElement,
    StatefulInteractiveElement as _, Styled, div, px,
};

use super::ChatPane;
use crate::app::prefs;

const SUMMARY_MAX: usize = 80;
const ARGS_LINES: usize = 10;
const OUTPUT_LINES: usize = 20;

impl ChatPane {
    /// The call a tool-result row at `ix` answers.
    fn call_for(&self, ix: usize) -> Option<&ToolCall> {
        let id = &self.turns.get(ix)?.outcome.as_ref()?.id;
        self.turns[..ix]
            .iter()
            .rev()
            .filter(|m| m.role == Role::Assistant)
            .flat_map(|m| &m.calls)
            .find(|c| &c.id == id)
    }

    fn call_line(&self, call: Option<&ToolCall>, name: &str, target: Option<&String>) -> String {
        let summary = call.map_or_else(|| name.to_owned(), summary);
        match target {
            Some(t) => format!("{summary}  @ {t}"),
            None => summary,
        }
    }

    /// A finished call: status, summary and target on one line; click to see the details.
    pub(super) fn call_row(&self, ix: usize, this: &Entity<Self>, cx: &App) -> AnyElement {
        let Some(turn) = self.turns.get(ix) else {
            return div().into_any_element();
        };
        let outcome = turn.outcome.as_ref();
        let failed = outcome.is_some_and(|o| o.is_error);
        let call = self.call_for(ix);
        let target = call.and_then(|c| self.targets.get(&c.id));
        let name = outcome.map_or("tool", |o| o.name.as_str());
        let open = self.expanded.contains(&ix);
        let theme = cx.theme();
        let (status, color) = if failed {
            (IconName::CircleAlert, theme.danger)
        } else {
            (IconName::CircleCheck, theme.success)
        };
        let chevron = if open {
            IconName::ChevronDown
        } else {
            IconName::ChevronRight
        };
        let muted = theme.muted_foreground;
        let hover = theme.muted.opacity(0.35);
        let this = this.clone();
        let line = h_flex()
            .id(("chat-call", ix))
            .gap_1p5()
            .ml(px(-4.0))
            .px_1()
            .py_0p5()
            .items_center()
            .rounded_md()
            .cursor_pointer()
            .text_xs()
            .text_color(muted)
            .hover(move |s| s.bg(hover))
            .on_click(move |_, _, cx| this.update(cx, |p, cx| p.toggle_call(ix, cx)))
            .child(Icon::new(chevron).xsmall())
            .child(Icon::new(status).xsmall().text_color(color))
            .child(
                div()
                    .flex_1()
                    .min_w_0()
                    .truncate()
                    .font_family(prefs::code_font_family(cx))
                    .child(self.call_line(call, name, target)),
            );
        let mut col = v_flex().child(line);
        if open {
            col = col.child(self.call_details(call, turn, failed, cx));
        }
        col.into_any_element()
    }

    fn call_details(
        &self,
        call: Option<&ToolCall>,
        turn: &Message,
        failed: bool,
        cx: &App,
    ) -> AnyElement {
        let theme = cx.theme();
        let output = if failed {
            theme.danger
        } else {
            theme.foreground
        };
        let mut col = v_flex()
            .gap_1()
            .mt_1()
            .px_2()
            .py_1()
            .rounded_md()
            .bg(theme.muted.opacity(0.25))
            .text_xs()
            .font_family(prefs::code_font_family(cx));
        if let Some(args) = call.map(args_text).filter(|a| !a.is_empty()) {
            col = col.child(
                div()
                    .text_color(theme.muted_foreground)
                    .line_clamp(ARGS_LINES)
                    .child(args),
            );
        }
        col.child(
            div()
                .text_color(output)
                .line_clamp(OUTPUT_LINES)
                .child(turn.content.clone()),
        )
        .into_any_element()
    }

    /// Calls the model just requested that have not finished.
    pub(super) fn running_calls(&self, cx: &App) -> AnyElement {
        let muted = cx.theme().muted_foreground;
        let calls = self.turns.last().map_or(&[][..], |m| m.calls.as_slice());
        v_flex()
            .children(calls.iter().map(|call| {
                h_flex()
                    .gap_1p5()
                    .items_center()
                    .text_xs()
                    .text_color(muted)
                    .child(Icon::new(IconName::Loader).xsmall())
                    .child(
                        div()
                            .flex_1()
                            .min_w_0()
                            .truncate()
                            .font_family(prefs::code_font_family(cx))
                            .child(self.call_line(
                                Some(call),
                                &call.name,
                                self.targets.get(&call.id),
                            )),
                    )
            }))
            .into_any_element()
    }

    pub(super) fn toggle_call(&mut self, ix: usize, cx: &mut Context<Self>) {
        if !self.expanded.remove(&ix) {
            self.expanded.insert(ix);
        }
        if let Some(row) = self.row_of_turn(ix) {
            self.scroller
                .update(cx, |s, cx| s.remeasure_items(row..row + 1, cx));
        }
        cx.notify();
    }
}

/// `name arg1 arg2`, clipped to one short line.
fn summary(call: &ToolCall) -> String {
    let values = call.args.as_object().into_iter().flat_map(|o| o.values());
    let mut out = call.name.clone();
    for v in values {
        out.push(' ');
        match v.as_str() {
            Some(s) => out.push_str(s),
            None => out.push_str(&v.to_string()),
        }
    }
    let out = out.replace('\n', " ");
    if out.chars().count() > SUMMARY_MAX {
        format!("{}...", out.chars().take(SUMMARY_MAX).collect::<String>())
    } else {
        out
    }
}

/// `key: value` per argument, values in full.
fn args_text(call: &ToolCall) -> String {
    call.args
        .as_object()
        .into_iter()
        .flatten()
        .map(|(k, v)| match v.as_str() {
            Some(s) => format!("{k}: {s}"),
            None => format!("{k}: {v}"),
        })
        .collect::<Vec<_>>()
        .join("\n")
}

#[cfg(test)]
mod tests {
    use serde_json::json;

    use super::*;

    fn call(args: serde_json::Value) -> ToolCall {
        ToolCall {
            id: "1".into(),
            name: "run_readonly_query".into(),
            args,
        }
    }

    #[test]
    fn summary_is_one_clipped_line() {
        let c = call(json!({ "query": format!("SELECT\n{}", "x".repeat(200)) }));
        let s = summary(&c);
        assert!(!s.contains('\n') && s.ends_with("..."));
        assert_eq!(summary(&call(json!({}))), "run_readonly_query");
    }

    #[test]
    fn args_keep_full_values() {
        let c = call(json!({ "query": "SELECT 1\nFROM t" }));
        assert_eq!(args_text(&c), "query: SELECT 1\nFROM t");
    }
}
