//! Filter bar: pick column, op, and value; build SQL or Mongo filter strings.

use gpui::{App, Context, Entity, IntoElement, Render, Window, div, prelude::*, px};
use gpui_component::{
    ActiveTheme, Sizable,
    button::{Button, ButtonVariants},
    h_flex,
    input::{Input, InputState},
};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum FilterOp {
    Eq,
    NotEq,
    Like,
    Gt,
    Lt,
    IsNull,
    IsNotNull,
}

impl FilterOp {
    pub fn label(&self) -> &'static str {
        match self {
            Self::Eq => "=",
            Self::NotEq => "≠",
            Self::Like => "contains",
            Self::Gt => ">",
            Self::Lt => "<",
            Self::IsNull => "is null",
            Self::IsNotNull => "is not null",
        }
    }

    pub fn has_value(&self) -> bool {
        !matches!(self, Self::IsNull | Self::IsNotNull)
    }

    fn next(self) -> Self {
        match self {
            Self::Eq => Self::NotEq,
            Self::NotEq => Self::Like,
            Self::Like => Self::Gt,
            Self::Gt => Self::Lt,
            Self::Lt => Self::IsNull,
            Self::IsNull => Self::IsNotNull,
            Self::IsNotNull => Self::Eq,
        }
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
            FilterOp::Lt => format!("{c} < '{}'", self.value.replace('\'', "''")),
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
            FilterOp::Lt => format!("{c} < '{}'", self.value.replace('\'', "''")),
        }
    }

    /// Alias for Postgres (tests / callers default).
    pub fn to_sql(&self) -> String {
        self.to_sql_postgres()
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
            FilterOp::Lt => format!("{{\"{}\":{{\"$lt\":\"{}\"}}}}", self.column, self.value),
        }
    }
}

pub struct FilterBar {
    pub columns: Vec<String>,
    pub selected_column: usize,
    pub op: FilterOp,
    value_input: Entity<InputState>,
}

impl FilterBar {
    pub fn new(window: &mut Window, cx: &mut Context<Self>, columns: Vec<String>) -> Self {
        let value_input = cx.new(|cx| InputState::new(window, cx));
        Self {
            columns,
            selected_column: 0,
            op: FilterOp::Eq,
            value_input,
        }
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

    pub fn clear(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.value_input.update(cx, |state, cx| {
            state.set_value("", window, cx);
        });
        cx.notify();
    }

    pub fn set_columns_if_empty(&mut self, columns: Vec<String>, cx: &mut Context<Self>) {
        if self.columns.is_empty() && !columns.is_empty() {
            self.columns = columns;
            self.selected_column = 0;
            cx.notify();
        }
    }

    fn cycle_column(&mut self) {
        if self.columns.is_empty() {
            return;
        }
        self.selected_column = (self.selected_column + 1) % self.columns.len();
    }

    fn cycle_op(&mut self) {
        self.op = self.op.next();
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
                    .label(
                        self.columns
                            .get(self.selected_column)
                            .cloned()
                            .unwrap_or_else(|| "—".into()),
                    )
                    .on_click(cx.listener(|fb, _, _, cx| {
                        fb.cycle_column();
                        cx.notify();
                    })),
            )
            .child(
                Button::new("fb-op")
                    .small()
                    .ghost()
                    .label(self.op.label())
                    .on_click(cx.listener(|fb, _, _, cx| {
                        fb.cycle_op();
                        cx.notify();
                    })),
            )
            .when(self.op.has_value(), |row| {
                row.child(
                    div()
                        .min_w(px(140.0))
                        .max_w(px(220.0))
                        .child(Input::new(&self.value_input).small().cleanable(true)),
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
        assert_eq!(expr.to_sql(), expr.to_sql_postgres());
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
}
