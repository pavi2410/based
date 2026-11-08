//! Filter bar: pick column, op, and value; build SQL or Mongo filter strings.

use std::collections::HashSet;

use gpui_kit::component::{
    ActiveTheme, Sizable, h_flex,
    input::InputEvent,
    input::{Input, InputState},
    tooltip::Tooltip,
    v_flex,
};
use gpui_kit::{
    App, Context, Entity, IntoElement, Render, SharedString, Styled, TestSupportExt as _, Window,
    div, prelude::*, px,
};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum FilterOp {
    Eq,
    NotEq,
    Like,
    NotLike,
    StartsWith,
    EndsWith,
    Gt,
    Gte,
    Lt,
    Lte,
    IsNull,
    IsNotNull,
    /// Array column: any element equals value.
    ArrayHas,
    /// Array column: contains all listed values.
    ArrayHasAll,
    /// Array column: shares any listed value.
    ArrayHasAny,
}

impl FilterOp {
    /// Extra operators offered for array columns.
    pub const ARRAY: [FilterOp; 3] = [Self::ArrayHas, Self::ArrayHasAll, Self::ArrayHasAny];

    /// Operators valid for a column of the given kind.
    pub fn for_column(is_array: bool) -> impl Iterator<Item = FilterOp> {
        let extra: &[FilterOp] = if is_array { &Self::ARRAY } else { &[] };
        Self::ALL.into_iter().chain(extra.iter().copied())
    }

    pub const ALL: [FilterOp; 12] = [
        Self::Eq,
        Self::NotEq,
        Self::Like,
        Self::NotLike,
        Self::StartsWith,
        Self::EndsWith,
        Self::Gt,
        Self::Gte,
        Self::Lt,
        Self::Lte,
        Self::IsNull,
        Self::IsNotNull,
    ];

    pub fn label(&self) -> &'static str {
        match self {
            Self::Eq => "=",
            Self::NotEq => "≠",
            Self::Like => "contains",
            Self::NotLike => "not contains",
            Self::StartsWith => "starts with",
            Self::EndsWith => "ends with",
            Self::Gt => ">",
            Self::Gte => "≥",
            Self::Lt => "<",
            Self::Lte => "≤",
            Self::IsNull => "is null",
            Self::IsNotNull => "is not null",
            Self::ArrayHas => "has",
            Self::ArrayHasAll => "has all",
            Self::ArrayHasAny => "has any",
        }
    }

    pub fn has_value(&self) -> bool {
        !matches!(self, Self::IsNull | Self::IsNotNull)
    }
}

fn quote_sql_ident(name: &str) -> String {
    format!("\"{}\"", name.replace('"', "\"\""))
}

#[derive(Clone, Debug)]
pub struct FilterExpr {
    pub column: String,
    pub op: FilterOp,
    pub value: String,
}

impl FilterExpr {
    /// `LIKE` pattern (`%`/`_`/`\\` escaped) for contains / starts / ends ops.
    fn like_pattern(&self) -> String {
        let v = self
            .value
            .replace('\\', "\\\\")
            .replace('%', "\\%")
            .replace('_', "\\_")
            .replace('\'', "''");
        match self.op {
            FilterOp::StartsWith => format!("{v}%"),
            FilterOp::EndsWith => format!("%{v}"),
            _ => format!("%{v}%"),
        }
    }

    /// Elements of a comma-separated list (or an existing `{..}` array literal).
    fn list_items(&self) -> Vec<String> {
        let v = self.value.trim();
        let inner = v
            .strip_prefix('{')
            .and_then(|r| r.strip_suffix('}'))
            .unwrap_or(v);
        inner
            .split(',')
            .map(|i| i.trim().trim_matches('"').to_string())
            .filter(|i| !i.is_empty())
            .collect()
    }

    /// Postgres array literal body for the listed values, SQL-escaped.
    fn pg_array_literal(&self) -> String {
        let items: Vec<String> = self
            .list_items()
            .iter()
            .map(|i| format!("\"{}\"", i.replace('\\', "\\\\").replace('"', "\\\"")))
            .collect();
        format!("{{{}}}", items.join(",")).replace('\'', "''")
    }

    /// Postgres-style fragment (`ILIKE`, quoted identifiers).
    pub fn to_sql_postgres(&self) -> String {
        let c = quote_sql_ident(&self.column);
        match self.op {
            FilterOp::IsNull => format!("{c} IS NULL"),
            FilterOp::IsNotNull => format!("{c} IS NOT NULL"),
            FilterOp::Like => format!("{c} ILIKE '%{}%'", self.value.replace('\'', "''")),
            FilterOp::Eq => format!("{c} = '{}'", self.value.replace('\'', "''")),
            FilterOp::NotEq => format!("{c} != '{}'", self.value.replace('\'', "''")),
            FilterOp::Gt => format!("{c} > '{}'", self.value.replace('\'', "''")),
            FilterOp::Gte => format!("{c} >= '{}'", self.value.replace('\'', "''")),
            FilterOp::Lt => format!("{c} < '{}'", self.value.replace('\'', "''")),
            FilterOp::Lte => format!("{c} <= '{}'", self.value.replace('\'', "''")),
            FilterOp::ArrayHas => format!("'{}' = ANY({c})", self.value.replace('\'', "''")),
            FilterOp::ArrayHasAll => format!("{c} @> '{}'", self.pg_array_literal()),
            FilterOp::ArrayHasAny => format!("{c} && '{}'", self.pg_array_literal()),
            FilterOp::NotLike => format!("{c} NOT ILIKE '{}'", self.like_pattern()),
            FilterOp::StartsWith | FilterOp::EndsWith => {
                format!("{c} ILIKE '{}'", self.like_pattern())
            }
        }
    }

    /// SQLite fragment (`LIKE`, quoted identifiers).
    pub fn to_sql_sqlite(&self) -> String {
        let c = quote_sql_ident(&self.column);
        match self.op {
            FilterOp::IsNull => format!("{c} IS NULL"),
            FilterOp::IsNotNull => format!("{c} IS NOT NULL"),
            FilterOp::Like => format!(
                "{c} LIKE '%{}%' ESCAPE '\\'",
                self.value
                    .replace('\\', "\\\\")
                    .replace('%', "\\%")
                    .replace('_', "\\_")
            ),
            FilterOp::Eq => format!("{c} = '{}'", self.value.replace('\'', "''")),
            FilterOp::NotEq => format!("{c} != '{}'", self.value.replace('\'', "''")),
            FilterOp::Gt => format!("{c} > '{}'", self.value.replace('\'', "''")),
            FilterOp::Gte => format!("{c} >= '{}'", self.value.replace('\'', "''")),
            FilterOp::Lt => format!("{c} < '{}'", self.value.replace('\'', "''")),
            FilterOp::Lte => format!("{c} <= '{}'", self.value.replace('\'', "''")),
            FilterOp::NotLike => {
                format!("{c} NOT LIKE '{}' ESCAPE '\\'", self.like_pattern())
            }
            // SQLite has no arrays; match the text form instead.
            FilterOp::ArrayHas | FilterOp::ArrayHasAll | FilterOp::ArrayHasAny => {
                format!("{c} LIKE '{}' ESCAPE '\\'", self.like_pattern())
            }
            FilterOp::StartsWith | FilterOp::EndsWith => {
                format!("{c} LIKE '{}' ESCAPE '\\'", self.like_pattern())
            }
        }
    }

    /// Value escaped as a regex literal inside a JSON string.
    fn mongo_regex(&self) -> String {
        let mut out = String::new();
        for ch in self.value.chars() {
            if ch == '\\' {
                out.push_str("\\\\\\\\");
                continue;
            }
            if ".^$*+?()[]{}|".contains(ch) {
                out.push_str("\\\\");
            } else if ch == '"' {
                out.push('\\');
            }
            out.push(ch);
        }
        out
    }

    /// MongoDB filter as extended-JSON string (single predicate).
    pub fn to_mongo_filter(&self) -> String {
        match self.op {
            FilterOp::IsNull => format!("{{\"{}\":null}}", self.column),
            FilterOp::IsNotNull => format!("{{\"{}\":{{\"$ne\":null}}}}", self.column),
            FilterOp::Eq => format!("{{\"{}\":\"{}\"}}", self.column, self.value),
            FilterOp::NotEq => format!("{{\"{}\":{{\"$ne\":\"{}\"}}}}", self.column, self.value),
            FilterOp::Like => format!(
                "{{\"{}\":{{\"$regex\":\"{}\",\"$options\":\"i\"}}}}",
                self.column,
                self.value.replace('\\', "\\\\").replace('"', "\\\"")
            ),
            FilterOp::Gt => format!("{{\"{}\":{{\"$gt\":\"{}\"}}}}", self.column, self.value),
            FilterOp::Gte => format!("{{\"{}\":{{\"$gte\":\"{}\"}}}}", self.column, self.value),
            FilterOp::Lt => format!("{{\"{}\":{{\"$lt\":\"{}\"}}}}", self.column, self.value),
            FilterOp::Lte => format!("{{\"{}\":{{\"$lte\":\"{}\"}}}}", self.column, self.value),
            FilterOp::ArrayHas => format!("{{\"{}\":\"{}\"}}", self.column, self.value),
            FilterOp::ArrayHasAll | FilterOp::ArrayHasAny => {
                let key = if self.op == FilterOp::ArrayHasAll {
                    "$all"
                } else {
                    "$in"
                };
                let items: Vec<String> = self
                    .list_items()
                    .iter()
                    .map(|i| format!("\"{}\"", i.replace('\\', "\\\\").replace('"', "\\\"")))
                    .collect();
                format!(
                    "{{\"{}\":{{\"{key}\":[{}]}}}}",
                    self.column,
                    items.join(",")
                )
            }
            FilterOp::NotLike => format!(
                "{{\"{}\":{{\"$not\":{{\"$regex\":\"{}\",\"$options\":\"i\"}}}}}}",
                self.column,
                self.mongo_regex()
            ),
            FilterOp::StartsWith => format!(
                "{{\"{}\":{{\"$regex\":\"^{}\",\"$options\":\"i\"}}}}",
                self.column,
                self.mongo_regex()
            ),
            FilterOp::EndsWith => format!(
                "{{\"{}\":{{\"$regex\":\"{}$\",\"$options\":\"i\"}}}}",
                self.column,
                self.mongo_regex()
            ),
        }
    }
}

/// How a term combines with the one before it. AND binds tighter than OR, as in SQL.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Join {
    #[default]
    And,
    Or,
}

impl Join {
    pub fn toggled(self) -> Self {
        match self {
            Self::And => Self::Or,
            Self::Or => Self::And,
        }
    }

    pub fn label(self) -> &'static str {
        match self {
            Self::And => "AND",
            Self::Or => "OR",
        }
    }
}

/// One predicate plus its connector to the previous term (ignored for the first).
#[derive(Clone, Debug)]
pub struct FilterTerm {
    pub join: Join,
    pub expr: FilterExpr,
}

/// A chain of predicates joined by AND / OR.
#[derive(Clone, Debug, Default)]
pub struct FilterSet {
    pub terms: Vec<FilterTerm>,
}

impl FilterSet {
    pub fn single(expr: FilterExpr) -> Self {
        Self {
            terms: vec![FilterTerm {
                join: Join::And,
                expr,
            }],
        }
    }

    pub fn is_empty(&self) -> bool {
        self.terms.is_empty()
    }

    /// Runs of AND-joined predicates, split at each OR.
    pub fn groups(&self) -> Vec<Vec<&FilterExpr>> {
        let mut groups: Vec<Vec<&FilterExpr>> = Vec::new();
        for (i, term) in self.terms.iter().enumerate() {
            if i == 0 || term.join == Join::Or {
                groups.push(Vec::new());
            }
            if let Some(g) = groups.last_mut() {
                g.push(&term.expr);
            }
        }
        groups
    }

    fn render_sql(&self, fragment: impl Fn(&FilterExpr) -> String) -> String {
        let groups = self.groups();
        let multi = groups.len() > 1;
        groups
            .iter()
            .map(|g| {
                let joined = g
                    .iter()
                    .map(|e| fragment(e))
                    .collect::<Vec<_>>()
                    .join(" AND ");
                if multi && g.len() > 1 {
                    format!("({joined})")
                } else {
                    joined
                }
            })
            .collect::<Vec<_>>()
            .join(" OR ")
    }

    /// `WHERE` body for Postgres (no leading keyword).
    pub fn to_sql_postgres(&self) -> String {
        self.render_sql(FilterExpr::to_sql_postgres)
    }

    /// `WHERE` body for SQLite (no leading keyword).
    pub fn to_sql_sqlite(&self) -> String {
        self.render_sql(FilterExpr::to_sql_sqlite)
    }

    /// MongoDB filter as extended JSON: `$or` of `$and` groups, collapsed when trivial.
    pub fn to_mongo_filter(&self) -> String {
        let groups = self.groups();
        let group_json: Vec<String> = groups
            .iter()
            .map(|g| match g.as_slice() {
                [only] => only.to_mongo_filter(),
                many => format!(
                    "{{\"$and\":[{}]}}",
                    many.iter()
                        .map(|e| e.to_mongo_filter())
                        .collect::<Vec<_>>()
                        .join(",")
                ),
            })
            .collect();
        match group_json.as_slice() {
            [] => "{}".into(),
            [only] => only.clone(),
            many => format!("{{\"$or\":[{}]}}", many.join(",")),
        }
    }
}

/// Emitted whenever the applied filter changes.
pub enum FilterEvent {
    Changed,
}

/// Operator spellings accepted when typing, longest first so `>=` wins over `>`.
const OP_ALIASES: &[(&str, FilterOp)] = &[
    ("is not null", FilterOp::IsNotNull),
    ("is null", FilterOp::IsNull),
    ("not contains", FilterOp::NotLike),
    ("starts with", FilterOp::StartsWith),
    ("ends with", FilterOp::EndsWith),
    ("has all", FilterOp::ArrayHasAll),
    ("has any", FilterOp::ArrayHasAny),
    ("contains", FilterOp::Like),
    ("like", FilterOp::Like),
    ("has", FilterOp::ArrayHas),
    (">=", FilterOp::Gte),
    ("<=", FilterOp::Lte),
    ("!=", FilterOp::NotEq),
    ("<>", FilterOp::NotEq),
    ("≠", FilterOp::NotEq),
    ("≥", FilterOp::Gte),
    ("≤", FilterOp::Lte),
    ("=", FilterOp::Eq),
    (">", FilterOp::Gt),
    ("<", FilterOp::Lt),
];

/// Read `column op value` out of typed text; `None` until it is complete.
pub fn parse_term(text: &str, columns: &[String]) -> Option<FilterExpr> {
    let text = text.trim();
    let (head, rest) = text
        .split_once(char::is_whitespace)
        .map(|(h, r)| (h, r.trim_start()))
        .or_else(|| {
            // `a=b` with no spaces: split at the first operator character.
            let i = text.find(['=', '<', '>', '!', '≠', '≥', '≤'])?;
            Some((&text[..i], &text[i..]))
        })?;
    let head = head.trim_matches('"');
    let column = columns
        .iter()
        .find(|c| c.eq_ignore_ascii_case(head))?
        .clone();
    let lower = rest.to_lowercase();
    let (alias, op) = OP_ALIASES.iter().find(|(a, _)| {
        lower.starts_with(a)
            && (!a.chars().all(char::is_alphabetic)
                || lower[a.len()..].starts_with(char::is_whitespace)
                || lower.len() == a.len())
    })?;
    let value = rest[alias.len()..].trim();
    let value = value
        .strip_prefix('\'')
        .and_then(|v| v.strip_suffix('\''))
        .unwrap_or(value)
        .to_string();
    if op.has_value() && value.is_empty() {
        return None;
    }
    Some(FilterExpr {
        column,
        op: *op,
        value: if op.has_value() { value } else { String::new() },
    })
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
                InputEvent::Change => cx.notify(),
                InputEvent::PressEnter { .. } => this.commit_or_accept(window, cx),
                _ => {}
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
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let theme = cx.theme();
        let (muted, fg, primary, border) = (
            theme.muted_foreground,
            theme.foreground,
            theme.primary,
            theme.border,
        );
        let value_color = theme.green_light;
        let text = self.input.read(cx).value().to_string();
        let suggestions = self.suggestions(&text);

        let mut row = h_flex()
            .id("filter-field")
            .relative()
            .flex_1()
            .min_w(px(280.0))
            .gap(px(6.0))
            .items_center()
            .flex_wrap()
            .px(px(8.0))
            .py(px(2.0))
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
                        h.child(div().text_color(value_color).child(e.value.clone()))
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
            div().flex_1().min_w(px(120.0)).child(
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
                    .top(px(36.0))
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sql_eq_postgres_quotes_ident() {
        let expr = FilterExpr {
            column: "email".into(),
            op: FilterOp::Eq,
            value: "a@b.com".into(),
        };
        assert_eq!(expr.to_sql_postgres(), "\"email\" = 'a@b.com'");
    }

    #[test]
    fn sql_like_postgres() {
        let expr = FilterExpr {
            column: "name".into(),
            op: FilterOp::Like,
            value: "alice".into(),
        };
        assert_eq!(expr.to_sql_postgres(), "\"name\" ILIKE '%alice%'");
    }

    #[test]
    fn sql_is_null() {
        let expr = FilterExpr {
            column: "deleted_at".into(),
            op: FilterOp::IsNull,
            value: String::new(),
        };
        assert_eq!(expr.to_sql_postgres(), "\"deleted_at\" IS NULL");
    }

    #[test]
    fn mongo_eq() {
        let expr = FilterExpr {
            column: "status".into(),
            op: FilterOp::Eq,
            value: "active".into(),
        };
        assert_eq!(expr.to_mongo_filter(), "{\"status\":\"active\"}");
    }

    #[test]
    fn sql_escapes_single_quote() {
        let expr = FilterExpr {
            column: "name".into(),
            op: FilterOp::Eq,
            value: "O'Brien".into(),
        };
        assert_eq!(expr.to_sql_postgres(), "\"name\" = 'O''Brien'");
    }

    #[test]
    fn sql_array_ops_postgres() {
        let mk = |op, v: &str| FilterExpr {
            column: "tags".into(),
            op,
            value: v.into(),
        };
        assert_eq!(
            mk(FilterOp::ArrayHas, "a").to_sql_postgres(),
            "'a' = ANY(\"tags\")"
        );
        assert_eq!(
            mk(FilterOp::ArrayHasAll, "a, b").to_sql_postgres(),
            "\"tags\" @> '{\"a\",\"b\"}'"
        );
        assert_eq!(
            mk(FilterOp::ArrayHasAny, "{1,2}").to_sql_postgres(),
            "\"tags\" && '{\"1\",\"2\"}'"
        );
    }

    fn expr(col: &str, op: FilterOp, v: &str) -> FilterExpr {
        FilterExpr {
            column: col.into(),
            op,
            value: v.into(),
        }
    }

    fn set(terms: &[(Join, FilterExpr)]) -> FilterSet {
        FilterSet {
            terms: terms
                .iter()
                .cloned()
                .map(|(join, expr)| FilterTerm { join, expr })
                .collect(),
        }
    }

    #[test]
    fn set_and_chain_has_no_parens() {
        let s = set(&[
            (Join::And, expr("plan", FilterOp::Eq, "team")),
            (Join::And, expr("seats", FilterOp::Gt, "10")),
        ]);
        assert_eq!(
            s.to_sql_postgres(),
            "\"plan\" = 'team' AND \"seats\" > '10'"
        );
    }

    #[test]
    fn set_or_binds_looser_and_groups_multi_term_runs() {
        let s = set(&[
            (Join::And, expr("a", FilterOp::Eq, "1")),
            (Join::And, expr("b", FilterOp::Eq, "2")),
            (Join::Or, expr("c", FilterOp::Eq, "3")),
        ]);
        assert_eq!(
            s.to_sql_sqlite(),
            "(\"a\" = '1' AND \"b\" = '2') OR \"c\" = '3'"
        );
    }

    #[test]
    fn set_mongo_shapes() {
        assert_eq!(FilterSet::default().to_mongo_filter(), "{}");
        let s = set(&[
            (Join::And, expr("a", FilterOp::Eq, "1")),
            (Join::Or, expr("b", FilterOp::Eq, "2")),
        ]);
        assert_eq!(
            s.to_mongo_filter(),
            "{\"$or\":[{\"a\":\"1\"},{\"b\":\"2\"}]}"
        );
        let both = set(&[
            (Join::And, expr("a", FilterOp::Eq, "1")),
            (Join::And, expr("b", FilterOp::Eq, "2")),
        ]);
        assert_eq!(
            both.to_mongo_filter(),
            "{\"$and\":[{\"a\":\"1\"},{\"b\":\"2\"}]}"
        );
    }

    fn cols() -> Vec<String> {
        vec!["plan".into(), "seats".into(), "deleted_at".into()]
    }

    #[test]
    fn parse_term_reads_spaced_and_tight_forms() {
        let e = parse_term("plan = team", &cols()).unwrap();
        assert_eq!(
            (e.column.as_str(), e.op, e.value.as_str()),
            ("plan", FilterOp::Eq, "team")
        );
        let e = parse_term("seats>=10", &cols()).unwrap();
        assert_eq!((e.op, e.value.as_str()), (FilterOp::Gte, "10"));
        let e = parse_term("PLAN contains 'te am'", &cols()).unwrap();
        assert_eq!((e.op, e.value.as_str()), (FilterOp::Like, "te am"));
    }

    #[test]
    fn parse_term_handles_null_ops_and_rejects_partial_input() {
        let e = parse_term("deleted_at is null", &cols()).unwrap();
        assert_eq!((e.op, e.value.as_str()), (FilterOp::IsNull, ""));
        assert!(parse_term("deleted_at is not null", &cols()).is_some());
        assert!(parse_term("plan =", &cols()).is_none());
        assert!(parse_term("plan", &cols()).is_none());
        assert!(parse_term("nope = 1", &cols()).is_none());
    }
}
