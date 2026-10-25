//! AI prompt bar wiring for [`QueryEditorPanel`]: context, applying proposals, run confirmation.

use based_ai::context::ContextLevel;
use gpui_kit::component::{
    ActiveTheme, Icon, IconName, Sizable as _,
    button::{Button, ButtonVariants as _},
    h_flex,
    menu::{DropdownMenu as _, PopupMenuItem},
};
use gpui_kit::{AnyElement, Context, IntoElement, ParentElement, Styled, Window, div};

use super::query_editor::{QueryEditorPanel, QueryStatus};
use crate::ai::{AiPromptEvent, ask_assistant, connection_ai, schema};
use crate::widgets::sql_editor::{set_input_text, text_from_input};

const FIX_LABEL: &str = "Fix the error";
const FIX_PROMPT: &str = "Fix this error without changing what the query does:";
const MAX_ERROR_CHARS: usize = 2000;
const PREVIEW_ROWS: usize = 20;
const PREVIEW_CELL_CHARS: usize = 80;

/// A question about the editor's query, answered in the assistant pane.
#[derive(Clone, Copy)]
pub(super) enum Assist {
    Explain,
    Optimize,
    Results,
}

impl Assist {
    pub(super) fn label(self) -> &'static str {
        match self {
            Self::Explain => "Explain Query",
            Self::Optimize => "Optimize Query",
            Self::Results => "Ask About Results",
        }
    }
}

impl QueryEditorPanel {
    fn ai_level(&self, cx: &gpui_kit::App) -> ContextLevel {
        schema::level(connection_ai(&self.conn_id, cx))
    }

    pub(super) fn toggle_ai(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let off = self.ai_level(cx) == ContextLevel::Off;
        let was_open = self.ai.is_open();
        self.ai.toggle(off, window, cx);
        if was_open {
            self.focus_editor(window, cx);
        }
    }

    fn focus_editor(&self, window: &mut Window, cx: &mut Context<Self>) {
        self.sql_input.update(cx, |s, cx| s.focus(window, cx));
    }

    /// Caret button next to "Ask AI" listing the query questions.
    pub(super) fn assist_menu(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let mut kinds = vec![Assist::Explain, Assist::Optimize];
        if self.can_ask_about_results(cx) {
            kinds.push(Assist::Results);
        }
        let panel = cx.weak_entity();
        Button::new(format!("{}-ai-more", self.source.id()))
            .ghost()
            .small()
            .dropdown_caret(true)
            .tooltip("Ask AI about this query")
            .dropdown_menu(move |mut menu, _, _| {
                for kind in &kinds {
                    let (kind, panel) = (*kind, panel.clone());
                    menu = menu.item(PopupMenuItem::new(kind.label()).on_click(move |_, _, cx| {
                        let _ = panel.update(cx, |p, cx| p.assist(kind, cx));
                    }));
                }
                menu
            })
    }

    /// `true` when the connection shares row data and there is a result to discuss.
    pub(super) fn can_ask_about_results(&self, cx: &gpui_kit::App) -> bool {
        self.ai_level(cx) == ContextLevel::SchemaRows
            && !self.result.read(cx).delegate().rows.is_empty()
    }

    /// Hands the selected query text (or the whole editor) to the assistant.
    pub(super) fn assist(&mut self, kind: Assist, cx: &mut Context<Self>) {
        let selected = self.sql_input.read(cx).selected_value().to_string();
        let sql = if selected.trim().is_empty() {
            text_from_input(&self.sql_input, cx)
        } else {
            selected
        };
        let text = match kind {
            Assist::Explain => {
                format!("Explain this query in plain language, step by step:\n```\n{sql}\n```")
            }
            Assist::Optimize => format!(
                "Review this query for performance. Check its plan with the explain tool if \
                 useful, and suggest a better version if there is one:\n```\n{sql}\n```"
            ),
            Assist::Results => format!(
                "This query:\n```\n{sql}\n```\nreturned:\n{}\nPoint out anything that stands out.",
                self.result_preview(cx)
            ),
        };
        ask_assistant(self.conn_id.clone(), text, cx);
    }

    /// The first rows of the result as ` | `-separated text.
    fn result_preview(&self, cx: &gpui_kit::App) -> String {
        let table = self.result.read(cx);
        let delegate = table.delegate();
        let clip = |cell: &str| -> String { cell.chars().take(PREVIEW_CELL_CHARS).collect() };
        let mut out = delegate
            .columns
            .iter()
            .map(|c| c.name.to_string())
            .collect::<Vec<_>>()
            .join(" | ");
        for row in delegate.rows.iter().take(PREVIEW_ROWS) {
            out.push('\n');
            out.push_str(&row.iter().map(|c| clip(c)).collect::<Vec<_>>().join(" | "));
        }
        let total = delegate.rows.len();
        if total > PREVIEW_ROWS {
            out.push_str(&format!(
                "\n({total} rows in total, first {PREVIEW_ROWS} shown)"
            ));
        }
        out
    }

    /// Opens the bar and asks the model to repair the query from the last error.
    pub(super) fn fix_with_ai(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let QueryStatus::Error(error) = &self.status else {
            return;
        };
        let error: String = error.chars().take(MAX_ERROR_CHARS).collect();
        if !self.ai.is_open() {
            self.toggle_ai(window, cx);
        }
        if self.ai_level(cx) == ContextLevel::Off {
            return;
        }
        self.ai
            .prompt()
            .update(cx, |ai, cx| ai.set_instruction(FIX_LABEL, window, cx));
        self.ask_ai(format!("{FIX_PROMPT}\n{error}"), cx);
    }

    pub(super) fn on_ai_event(
        &mut self,
        event: &AiPromptEvent,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        match event {
            AiPromptEvent::Submit(instruction) => self.ask_ai(instruction.clone(), cx),
            AiPromptEvent::Apply(text) => {
                self.ai.note_applied(text);
                self.ai_confirm = false;
                set_input_text(&self.sql_input, text, window, cx);
                self.ai.close(cx);
                self.focus_editor(window, cx);
            }
            AiPromptEvent::Close => {
                self.ai.close(cx);
                self.focus_editor(window, cx);
            }
        }
    }

    fn ask_ai(&mut self, instruction: String, cx: &mut Context<Self>) {
        let engine = self.source.engine();
        let current = text_from_input(&self.sql_input, cx);
        let mentioned = format!("{current} {instruction}");
        let schema = schema::context(engine, &self.schema, self.ai_level(cx), &mentioned);
        self.ai.ask(engine, schema, current, instruction, cx);
    }

    /// `true` when the run was held back for confirmation.
    pub(super) fn gate_ai_run(&mut self, sql: &str, cx: &mut Context<Self>) -> bool {
        let hold = self.ai.needs_confirm(self.source.engine(), sql);
        if self.ai_confirm != hold {
            self.ai_confirm = hold;
            cx.notify();
        }
        hold
    }

    fn confirm_ai_run(&mut self, cx: &mut Context<Self>) {
        self.ai.confirmed();
        self.ai_confirm = false;
        self.run(cx);
    }

    pub(super) fn render_ai_confirm(&self, cx: &mut Context<Self>) -> Option<AnyElement> {
        if !self.ai_confirm {
            return None;
        }
        let id = self.source.id();
        let warn = cx.theme().warning;
        Some(
            h_flex()
                .gap_2()
                .px_2()
                .py_1p5()
                .items_center()
                .border_b_1()
                .border_color(warn.opacity(0.4))
                .bg(warn.opacity(0.08))
                .child(Icon::new(IconName::TriangleAlert).small().text_color(warn))
                .child(
                    div()
                        .flex_1()
                        .text_sm()
                        .child("This query came from AI and may change data or schema. Run it?"),
                )
                .child(
                    Button::new(format!("{id}-ai-run"))
                        .primary()
                        .small()
                        .label("Run anyway")
                        .on_click(cx.listener(|this, _, _, cx| this.confirm_ai_run(cx))),
                )
                .child(
                    Button::new(format!("{id}-ai-cancel"))
                        .ghost()
                        .small()
                        .label("Cancel")
                        .on_click(cx.listener(|this, _, _, cx| {
                            this.ai_confirm = false;
                            cx.notify();
                        })),
                )
                .into_any_element(),
        )
    }
}
