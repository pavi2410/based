//! Filter bar: pick column, op, and value; build SQL or Mongo filter strings.

use gpui::{Context, IntoElement, Render, Window, div, prelude::*};
use gpui_component::{ActiveTheme, h_flex};

#[derive(Clone, Debug, PartialEq, Eq)]
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
}

#[derive(Clone, Debug)]
pub struct FilterExpr {
    pub column: String,
    pub op: FilterOp,
    pub value: String,
}

impl FilterExpr {
    /// Generate a SQL WHERE clause fragment (parameterized placeholder as literal for display).
    pub fn to_sql(&self) -> String {
        match self.op {
            FilterOp::IsNull => format!("{} IS NULL", self.column),
            FilterOp::IsNotNull => format!("{} IS NOT NULL", self.column),
            FilterOp::Like => format!(
                "{} ILIKE '%{}%'",
                self.column,
                self.value.replace('\'', "''")
            ),
            FilterOp::Eq => format!("{} = '{}'", self.column, self.value.replace('\'', "''")),
            FilterOp::NotEq => format!("{} != '{}'", self.column, self.value.replace('\'', "''")),
            FilterOp::Gt => format!("{} > '{}'", self.column, self.value.replace('\'', "''")),
            FilterOp::Lt => format!("{} < '{}'", self.column, self.value.replace('\'', "''")),
        }
    }

    /// Generate a MongoDB filter document fragment (as JSON string).
    pub fn to_mongo_filter(&self) -> String {
        match self.op {
            FilterOp::IsNull => format!("{{\"{}\":null}}", self.column),
            FilterOp::IsNotNull => format!("{{\"{}\":{{\"$ne\":null}}}}", self.column),
            FilterOp::Eq => format!("{{\"{}\":\"{}\"}}", self.column, self.value),
            FilterOp::NotEq => format!("{{\"{}\":{{\"$ne\":\"{}\"}}}}", self.column, self.value),
            FilterOp::Like => format!(
                "{{\"{}\":{{\"$regex\":\"{}\",\"$options\":\"i\"}}}}",
                self.column, self.value
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
    pub value: String,
    pub active: bool,
}

impl FilterBar {
    pub fn new(columns: Vec<String>) -> Self {
        Self {
            columns,
            selected_column: 0,
            op: FilterOp::Eq,
            value: String::new(),
            active: false,
        }
    }

    pub fn current_expr(&self) -> Option<FilterExpr> {
        if !self.active || self.columns.is_empty() {
            return None;
        }
        let col = self.columns.get(self.selected_column)?.clone();
        Some(FilterExpr {
            column: col,
            op: self.op.clone(),
            value: self.value.clone(),
        })
    }

    pub fn clear(&mut self) {
        self.active = false;
        self.value.clear();
    }
}

impl Render for FilterBar {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let theme = cx.theme();
        h_flex()
            .gap_1()
            .text_xs()
            .text_color(theme.muted_foreground)
            .child(div().child("Filter:"))
            .child(
                div().px_2().py_1().bg(theme.secondary).rounded(gpui::px(4.0)).child(
                    self.columns
                        .get(self.selected_column)
                        .cloned()
                        .unwrap_or_default(),
                ),
            )
            .child(
                div()
                    .px_2()
                    .py_1()
                    .bg(theme.secondary)
                    .rounded(gpui::px(4.0))
                    .child(self.op.label()),
            )
            .when(self.op.has_value(), |d| {
                d.child(
                    div()
                        .px_2()
                        .py_1()
                        .bg(theme.input)
                        .border_1()
                        .border_color(theme.border)
                        .rounded(gpui::px(4.0))
                        .min_w(gpui::px(100.0))
                        .child(if self.value.is_empty() {
                            "value…".to_string()
                        } else {
                            self.value.clone()
                        }),
                )
            })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sql_eq() {
        let expr = FilterExpr {
            column: "email".into(),
            op: FilterOp::Eq,
            value: "a@b.com".into(),
        };
        assert_eq!(expr.to_sql(), "email = 'a@b.com'");
    }

    #[test]
    fn sql_like() {
        let expr = FilterExpr {
            column: "name".into(),
            op: FilterOp::Like,
            value: "alice".into(),
        };
        assert_eq!(expr.to_sql(), "name ILIKE '%alice%'");
    }

    #[test]
    fn sql_is_null() {
        let expr = FilterExpr {
            column: "deleted_at".into(),
            op: FilterOp::IsNull,
            value: String::new(),
        };
        assert_eq!(expr.to_sql(), "deleted_at IS NULL");
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
        assert_eq!(expr.to_sql(), "name = 'O''Brien'");
    }
}
