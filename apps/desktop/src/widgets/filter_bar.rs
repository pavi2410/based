//! Filter bar: pick column, op, and value; build SQL or Mongo filter strings.

use std::collections::HashSet;
use std::time::Duration;

use gpui_kit::component::{
    ActiveTheme, Sizable, h_flex,
    input::InputEvent,
    input::{Input, InputState},
    tooltip::Tooltip,
    v_flex,
};
use gpui_kit::{
    App, Context, Entity, Focusable as _, IntoElement, Render, SharedString, Styled,
    TestSupportExt as _, Window, div, prelude::*, px,
};

pub use based_query::filter::{FilterExpr, FilterOp, FilterSet, FilterTerm, Join};
use based_query::filter::{OP_ALIASES, parse_term};

/// Emitted whenever the applied filter changes.
pub enum FilterEvent {
    Changed,
}

/// A filter field made of editable predicate "islands" joined by AND / OR.
pub struct FilterBar {
    pub columns: Vec<String>,
    terms: Vec<FilterTerm>,
    input: Entity<InputState>,
    /// Columns whose type is an array (enables array operators).
    array_columns: HashSet<String>,
    /// `columns` holds only a preset column until the real list loads.
    preset_only: bool,
}

impl gpui_kit::EventEmitter<FilterEvent> for FilterBar {}

impl FilterBar {
    pub fn new(window: &mut Window, cx: &mut Context<Self>, columns: Vec<String>) -> Self {
        let input = cx.new(|cx| {
            InputState::new(window, cx).placeholder("column, operator, value — e.g. plan = team")
        });
        cx.subscribe_in(
            &input,
            window,
            |this, _, event: &InputEvent, window, cx| match event {
                InputEvent::Change | InputEvent::Focus => cx.notify(),
                InputEvent::Blur => {
                    // Wait out a click on a suggestion before hiding the list.
                    cx.spawn(async move |this, cx| {
                        cx.background_executor()
                            .timer(Duration::from_millis(150))
                            .await;
                        let _ = this.update(cx, |_, cx| cx.notify());
                    })
                    .detach();
                }
                InputEvent::PressEnter { .. } => this.commit_or_accept(window, cx),
            },
        )
        .detach();
        Self {
            columns,
            terms: Vec::new(),
            input,
            array_columns: HashSet::new(),
            preset_only: false,
        }
    }

    /// Start with `column = value`; the full column list replaces the preset on load.
    pub fn preset_eq(
        &mut self,
        column: String,
        value: &str,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.preset(column, FilterOp::Eq, value, window, cx);
    }

    /// Replace the filter with `column <op> value`; the caller reloads.
    pub fn preset(
        &mut self,
        column: String,
        op: FilterOp,
        value: &str,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if !self.columns.contains(&column) {
            self.columns = vec![column.clone()];
            self.preset_only = true;
        }
        self.terms = vec![FilterTerm {
            join: Join::And,
            expr: FilterExpr {
                column,
                op,
                value: if op.has_value() {
                    value.to_string()
                } else {
                    String::new()
                },
            },
        }];
        self.input
            .update(cx, |state, cx| state.set_value("", window, cx));
        cx.notify();
    }

    /// Put `column ` in the field, ready for an operator.
    pub fn start_term(&mut self, column: &str, window: &mut Window, cx: &mut Context<Self>) {
        self.set_input(&format!("{column} "), window, cx);
        cx.notify();
    }

    /// Append `expr` joined with AND and apply it.
    pub fn add_term(&mut self, expr: FilterExpr, cx: &mut Context<Self>) {
        if !self.columns.contains(&expr.column) {
            self.columns.push(expr.column.clone());
        }
        self.terms.push(FilterTerm {
            join: Join::And,
            expr,
        });
        self.changed(cx);
    }

    /// The applied conditions; `None` when there are none.
    pub fn current_set(&self, _cx: &App) -> Option<FilterSet> {
        (!self.terms.is_empty()).then(|| FilterSet {
            terms: self.terms.clone(),
        })
    }

    pub fn set_array_columns(&mut self, cols: HashSet<String>) {
        self.array_columns = cols;
    }

    pub fn clear(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.terms.clear();
        self.input
            .update(cx, |state, cx| state.set_value("", window, cx));
        cx.notify();
    }

    pub fn set_columns_if_empty(&mut self, columns: Vec<String>, cx: &mut Context<Self>) {
        if (self.columns.is_empty() || self.preset_only) && !columns.is_empty() {
            self.columns = columns;
            self.preset_only = false;
            cx.notify();
        }
    }

    fn changed(&mut self, cx: &mut Context<Self>) {
        cx.emit(FilterEvent::Changed);
        cx.notify();
    }

    fn set_input(&mut self, text: &str, window: &mut Window, cx: &mut Context<Self>) {
        self.input.update(cx, |state, cx| {
            state.set_value(text, window, cx);
            state.focus(window, cx);
        });
    }

    fn commit_or_accept(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let text = self.input.read(cx).value().to_string();
        if let Some(expr) = parse_term(&text, &self.columns) {
            self.terms.push(FilterTerm {
                join: Join::And,
                expr,
            });
            self.set_input("", window, cx);
            self.changed(cx);
        } else if let Some(first) = self.suggestions(&text).into_iter().next() {
            self.set_input(&first, window, cx);
        }
    }

    /// Completion texts for the slot being typed: column, then operator.
    fn suggestions(&self, text: &str) -> Vec<String> {
        let lower = text.to_lowercase();
        let Some((head, rest)) = text.split_once(char::is_whitespace) else {
            return self
                .columns
                .iter()
                .filter(|c| c.to_lowercase().starts_with(&lower))
                .take(8)
                .map(|c| format!("{c} "))
                .collect();
        };
        let Some(column) = self.columns.iter().find(|c| c.eq_ignore_ascii_case(head)) else {
            return Vec::new();
        };
        let rest = rest.trim_start().to_lowercase();
        if OP_ALIASES
            .iter()
            .any(|(a, _)| rest.starts_with(a) && rest.len() > a.len())
        {
            return Vec::new(); // typing the value
        }
        let is_array = self.array_columns.contains(column);
        FilterOp::for_column(is_array)
            .filter(|op| op.label().starts_with(rest.as_str()) && op.label() != rest)
            .take(8)
            .map(|op| {
                if op.has_value() {
                    format!("{column} {} ", op.label())
                } else {
                    format!("{column} {}", op.label())
                }
            })
            .collect()
    }
}

impl Render for FilterBar {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let theme = cx.theme();
        let (muted, fg, primary, border) = (
            theme.muted_foreground,
            theme.foreground,
            theme.primary,
            theme.border,
        );
        let value_color = theme.green_light;
        let text = self.input.read(cx).value().to_string();
        // Completions only while the field has focus, so they never float over the grid.
        let focused = self.input.focus_handle(cx).is_focused(window);
        let suggestions = if focused {
            self.suggestions(&text)
        } else {
            Vec::new()
        };

        let mut row = h_flex()
            .id("filter-field")
            .relative()
            .flex_1()
            .min_w(px(280.0))
            .min_h(px(30.0))
            .gap(px(6.0))
            .items_center()
            .flex_wrap()
            .px(px(8.0))
            .py(px(1.0))
            .rounded(px(8.0))
            .border_1()
            .border_color(border)
            .bg(theme.background)
            .child(
                div()
                    .text_xs()
                    .font_weight(gpui_kit::FontWeight::SEMIBOLD)
                    .text_color(muted)
                    .child("WHERE"),
            );

        let groups_multi = self.terms.iter().skip(1).any(|t| t.join == Join::Or);
        for (i, term) in self.terms.iter().enumerate() {
            if i > 0 {
                let join = term.join;
                row = row.child(
                    div()
                        .id(("filter-join", i))
                        .px(px(4.0))
                        .rounded(px(4.0))
                        .cursor_pointer()
                        .text_xs()
                        .font_weight(gpui_kit::FontWeight::SEMIBOLD)
                        .text_color(if join == Join::Or {
                            theme.warning
                        } else {
                            muted
                        })
                        .hover(move |s| s.bg(border.opacity(0.5)))
                        .tooltip(move |window, cx| {
                            Tooltip::new("Switch AND / OR").build(window, cx)
                        })
                        .child(join.label())
                        .on_click(cx.listener(move |this, _, _, cx| {
                            if let Some(t) = this.terms.get_mut(i) {
                                t.join = t.join.toggled();
                            }
                            this.changed(cx);
                        })),
                );
            }
            let e = &term.expr;
            let label = e.op.label();
            let _ = groups_multi;
            row = row.child(
                h_flex()
                    .id(("filter-island", i))
                    .gap(px(5.0))
                    .items_center()
                    .pl(px(8.0))
                    .pr(px(2.0))
                    .rounded(px(6.0))
                    .border_1()
                    .border_color(primary.opacity(0.35))
                    .bg(primary.opacity(0.1))
                    .cursor_pointer()
                    .text_xs()
                    .child(div().text_color(fg).child(e.column.clone()))
                    .child(div().text_color(primary).child(label))
                    .when(e.op.has_value(), |h| {
                        h.child(
                            div()
                                .max_w(px(300.0))
                                .truncate()
                                .text_color(value_color)
                                .child(e.value.clone()),
                        )
                    })
                    .child(
                        div()
                            .id(SharedString::from(format!("filter-island-x-{i}")))
                            .test_support()
                            .size(px(16.0))
                            .rounded_full()
                            .flex()
                            .items_center()
                            .justify_center()
                            .text_color(muted)
                            .hover(move |s| s.bg(border))
                            .child("×")
                            .on_click(cx.listener(move |this, _, _, cx| {
                                if i < this.terms.len() {
                                    this.terms.remove(i);
                                }
                                this.changed(cx);
                            })),
                    )
                    .on_click(cx.listener(move |this, _, window, cx| {
                        // Click an island to edit it as text.
                        if i < this.terms.len() {
                            let t = this.terms.remove(i);
                            let e = &t.expr;
                            let text = if e.op.has_value() {
                                format!("{} {} {}", e.column, e.op.label(), e.value)
                            } else {
                                format!("{} {}", e.column, e.op.label())
                            };
                            this.set_input(&text, window, cx);
                            this.changed(cx);
                        }
                    })),
            );
        }

        row = row.child(
            div().flex_1().min_w(px(140.0)).child(
                Input::new(&self.input)
                    .small()
                    .appearance(false)
                    .cleanable(false),
            ),
        );
        if !suggestions.is_empty() {
            row = row.child(
                v_flex()
                    .absolute()
                    .top_full()
                    .mt(px(4.0))
                    .left(px(0.0))
                    .min_w(px(220.0))
                    .p(px(4.0))
                    .rounded(px(8.0))
                    .border_1()
                    .border_color(border)
                    .bg(theme.popover)
                    .shadow_md()
                    .occlude()
                    .children(suggestions.into_iter().enumerate().map(|(i, text)| {
                        div()
                            .id(("filter-suggest", i))
                            .px(px(8.0))
                            .py(px(3.0))
                            .rounded(px(5.0))
                            .text_xs()
                            .cursor_pointer()
                            .hover(move |s| s.bg(border.opacity(0.5)))
                            .child(text.clone())
                            .on_click(cx.listener(move |this, _, window, cx| {
                                this.set_input(&text, window, cx);
                            }))
                    })),
            );
        }
        row
    }
}
