//! Filter bar: pick column, op, and value; build SQL or Mongo filter strings.

use std::collections::HashSet;

use gpui_kit::component::{
    ActiveTheme, Sizable,
    button::{Button, ButtonVariants},
    h_flex,
    input::{Input, InputState},
    menu::{DropdownMenu as _, PopupMenuItem},
};
use gpui_kit::{App, Context, Entity, IntoElement, Render, Styled, Window, div, prelude::*, px};

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

pub struct FilterBar {
    pub columns: Vec<String>,
    pub selected_column: usize,
    pub op: FilterOp,
    value_input: Entity<InputState>,
    /// Columns whose type is an array (enables array operators).
    array_columns: HashSet<String>,
    /// `columns` holds only a preset column until the real list loads.
    preset_only: bool,
}

impl FilterBar {
    pub fn new(window: &mut Window, cx: &mut Context<Self>, columns: Vec<String>) -> Self {
        let value_input = cx.new(|cx| InputState::new(window, cx));
        Self {
            columns,
            selected_column: 0,
            op: FilterOp::Eq,
            value_input,
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

    /// Start with `column <op> value`; the full column list replaces the preset on load.
    pub fn preset(
        &mut self,
        column: String,
        op: FilterOp,
        value: &str,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.columns = vec![column];
        self.selected_column = 0;
        self.op = op;
        self.preset_only = true;
        self.value_input
            .update(cx, |state, cx| state.set_value(value, window, cx));
    }

    pub fn current_expr(&self, cx: &App) -> Option<FilterExpr> {
        if self.columns.is_empty() {
            return None;
        }
        let col = self.columns.get(self.selected_column)?.clone();
        let value = self.value_input.read(cx).value().to_string();
        match self.op {
            FilterOp::IsNull | FilterOp::IsNotNull => Some(FilterExpr {
                column: col,
                op: self.op,
                value: String::new(),
            }),
            _ => {
                if value.trim().is_empty() {
                    None
                } else {
                    Some(FilterExpr {
                        column: col,
                        op: self.op,
                        value,
                    })
                }
            }
        }
    }

    pub fn set_array_columns(&mut self, cols: HashSet<String>) {
        self.array_columns = cols;
    }

    fn selected_is_array(&self) -> bool {
        self.columns
            .get(self.selected_column)
            .is_some_and(|c| self.array_columns.contains(c))
    }

    pub fn clear(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.value_input.update(cx, |state, cx| {
            state.set_value("", window, cx);
        });
        cx.notify();
    }

    pub fn set_columns_if_empty(&mut self, columns: Vec<String>, cx: &mut Context<Self>) {
        if (self.columns.is_empty() || self.preset_only) && !columns.is_empty() {
            self.selected_column = self
                .columns
                .get(self.selected_column)
                .and_then(|c| columns.iter().position(|n| n == c))
                .unwrap_or(0);
            self.columns = columns;
            self.preset_only = false;
            cx.notify();
        }
    }
}

impl Render for FilterBar {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let theme = cx.theme();
        h_flex()
            .gap_1()
            .items_center()
            .text_xs()
            .text_color(theme.muted_foreground)
            .child(div().child("Filter"))
            .child(
                Button::new("fb-col")
                    .small()
                    .ghost()
                    .dropdown_caret(true)
                    .label(
                        self.columns
                            .get(self.selected_column)
                            .cloned()
                            .unwrap_or_else(|| "—".into()),
                    )
                    .dropdown_menu({
                        let (fb, columns, current) =
                            (cx.weak_entity(), self.columns.clone(), self.selected_column);
                        move |mut menu, _, _| {
                            for (i, name) in columns.iter().enumerate() {
                                let fb = fb.clone();
                                menu = menu.item(
                                    PopupMenuItem::new(name.clone())
                                        .checked(i == current)
                                        .on_click(move |_, _, cx| {
                                            let _ = fb.update(cx, |fb, cx| {
                                                fb.selected_column = i;
                                                if !fb.selected_is_array()
                                                    && FilterOp::ARRAY.contains(&fb.op)
                                                {
                                                    fb.op = FilterOp::Eq;
                                                }
                                                cx.notify();
                                            });
                                        }),
                                );
                            }
                            menu
                        }
                    }),
            )
            .child(
                Button::new("fb-op")
                    .small()
                    .ghost()
                    .dropdown_caret(true)
                    .label(self.op.label())
                    .dropdown_menu({
                        let (fb, current, is_array) =
                            (cx.weak_entity(), self.op, self.selected_is_array());
                        move |mut menu, _, _| {
                            for op in FilterOp::for_column(is_array) {
                                let fb = fb.clone();
                                menu = menu.item(
                                    PopupMenuItem::new(op.label())
                                        .checked(op == current)
                                        .on_click(move |_, _, cx| {
                                            let _ = fb.update(cx, |fb, cx| {
                                                fb.op = op;
                                                cx.notify();
                                            });
                                        }),
                                );
                            }
                            menu
                        }
                    }),
            )
            .when(self.op.has_value(), |row| {
                row.child(
                    h_flex()
                        .items_center()
                        .h(px(24.0))
                        .min_w(px(140.0))
                        .max_w(px(220.0))
                        .child(
                            Input::new(&self.value_input)
                                .small()
                                .cleanable(true)
                                .shadow_none(),
                        ),
                )
            })
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
}
