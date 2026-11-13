//! Tool calls in the transcript: one status line each, expanding to arguments and output.

use based_ai::tools::{CONNECT, LIST_CONNECTIONS, RUN_QUERY, ToolCall};
use based_ai::{Message, MessageBody};
use gpui_kit::component::{ActiveTheme, Icon, IconName, Sizable as _, h_flex, v_flex};
use gpui_kit::{
    AnyElement, App, Context, Entity, InteractiveElement as _, IntoElement, ParentElement,
    SharedString, StatefulInteractiveElement as _, Styled, div, px,
};

use super::{ChatPane, style};
use crate::app::prefs;
use crate::widgets::motion;

const SUMMARY_MAX: usize = 80;
const ARGS_LINES: usize = 10;
const OUTPUT_LINES: usize = 20;

impl ChatPane {
    /// The call a tool-result row at `ix` answers.
    fn call_for(&self, ix: usize) -> Option<&ToolCall> {
        let MessageBody::Tool { call_id, .. } = &self.turns.get(ix)?.body else {
            return None;
        };
        self.turns[..ix]
            .iter()
            .rev()
            .flat_map(Message::calls)
            .find(|c| &c.id == call_id)
    }

    fn call_line(
        &self,
        call: Option<&ToolCall>,
        name: &str,
        target: Option<&String>,
        running: bool,
    ) -> String {
        let summary = call.map_or_else(|| name.to_owned(), |c| sentence(c, running));
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
        let (name, failed) = match &turn.body {
            MessageBody::Tool { name, result, .. } => (name.as_str(), result.is_error),
            _ => ("tool", false),
        };
        let call = self.call_for(ix);
        let target = call.and_then(|c| self.targets.get(&c.id));
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
            .text_size(style::small(cx))
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
                    .child(self.call_line(call, name, target, false)),
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
            .text_size(style::small(cx))
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
                .child(turn.content().to_string()),
        )
        .into_any_element()
    }

    /// Calls the model just requested that have not finished.
    pub(super) fn running_calls(&self, cx: &App) -> AnyElement {
        let muted = cx.theme().muted_foreground;
        let calls = self.turns.last().map_or(&[][..], Message::calls);
        v_flex()
            .children(calls.iter().map(|call| {
                let row = h_flex()
                    .gap_1p5()
                    .items_center()
                    .text_size(style::small(cx))
                    .text_color(muted)
                    .child(Icon::new(IconName::Loader).xsmall())
                    .child(div().flex_1().min_w_0().truncate().child(self.call_line(
                        Some(call),
                        &call.name,
                        self.targets.get(&call.id),
                        true,
                    )));
                motion::pulse(SharedString::from(format!("chat-run-{}", call.id)), row)
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

/// What the call does, in words: "Looked at orders", or "Looking at orders" while it runs.
fn sentence(call: &ToolCall, running: bool) -> String {
    let arg = |key: &str| call.args[key].as_str().unwrap_or_default().to_owned();
    let (doing, did) = match call.name.as_str() {
        "list_tables" => ("Listing tables".to_owned(), "Listed tables".to_owned()),
        "describe_table" => {
            let t = arg("table");
            (format!("Looking at {t}"), format!("Looked at {t}"))
        }
        "explain" => (
            "Checking the plan".to_owned(),
            "Checked the plan".to_owned(),
        ),
        RUN_QUERY => ("Running a query".to_owned(), "Ran a query".to_owned()),
        LIST_CONNECTIONS => (
            "Listing connections".to_owned(),
            "Listed connections".to_owned(),
        ),
        CONNECT => {
            let c = arg("connection");
            (format!("Connecting to {c}"), format!("Connected to {c}"))
        }
        _ => {
            let s = summary(call);
            return s;
        }
    };
    if running { doing } else { did }
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
    fn steps_read_as_sentences() {
        let c = ToolCall {
            id: "1".into(),
            name: "describe_table".into(),
            args: json!({ "table": "orders" }),
        };
        assert_eq!(sentence(&c, true), "Looking at orders");
        assert_eq!(sentence(&c, false), "Looked at orders");
        assert_eq!(sentence(&call(json!({})), false), "Ran a query");
    }

    #[test]
    fn args_keep_full_values() {
        let c = call(json!({ "query": "SELECT 1\nFROM t" }));
        assert_eq!(args_text(&c), "query: SELECT 1\nFROM t");
    }
}
