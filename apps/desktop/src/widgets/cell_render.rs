//! Schema-driven grid cell styling (numeric color, boolean icons).

use gpui::{App, IntoElement, SharedString, Window, div, prelude::*};
use gpui_component::{
    ActiveTheme, Icon, IconName, Sizable as _, StyleSized, h_flex, tooltip::Tooltip,
};

use crate::app::prefs;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ColumnValueKind {
    Numeric,
    Boolean,
    Text,
    Unknown,
}

/// Lowercase base SQL type with parenthetical suffix stripped (`numeric(10,2)` → `numeric`).
pub fn normalize_type_name(raw: &str) -> String {
    let lower = raw.trim().to_ascii_lowercase();
    let base = lower.split('(').next().unwrap_or(&lower).trim();
    base.trim_end_matches("[]").to_string()
}

pub fn column_value_kind(data_type: Option<&str>) -> ColumnValueKind {
    let Some(raw) = data_type else {
        return ColumnValueKind::Unknown;
    };
    if raw.trim().is_empty() {
        return ColumnValueKind::Unknown;
    }

    let base = normalize_type_name(raw);
    if is_numeric_type(&base) {
        return ColumnValueKind::Numeric;
    }
    if is_boolean_type(&base) {
        return ColumnValueKind::Boolean;
    }
    if is_text_type(&base) {
        return ColumnValueKind::Text;
    }
    ColumnValueKind::Unknown
}

fn is_numeric_type(base: &str) -> bool {
    matches!(
        base,
        "int"
            | "int2"
            | "int4"
            | "int8"
            | "integer"
            | "bigint"
            | "smallint"
            | "serial"
            | "bigserial"
            | "numeric"
            | "decimal"
            | "float"
            | "float4"
            | "float8"
            | "real"
            | "double precision"
            | "money"
            | "number"
    )
}

fn is_boolean_type(base: &str) -> bool {
    matches!(base, "bool" | "boolean")
}

fn is_text_type(base: &str) -> bool {
    matches!(
        base,
        "text"
            | "varchar"
            | "character varying"
            | "char"
            | "character"
            | "name"
            | "json"
            | "jsonb"
            | "uuid"
            | "bytea"
            | "blob"
            | "date"
            | "timestamp"
            | "timestamptz"
            | "time"
            | "datetime"
            | "timestamp without time zone"
            | "timestamp with time zone"
            | "time without time zone"
            | "time with time zone"
    )
}

/// Parse display strings for boolean columns (`true`/`false`/`t`/`f`).
pub fn parse_bool_display(s: &str) -> Option<bool> {
    let t = s.trim();
    if t.eq_ignore_ascii_case("true") || t == "t" {
        Some(true)
    } else if t.eq_ignore_ascii_case("false") || t == "f" {
        Some(false)
    } else {
        None
    }
}

fn cell_chrome(cx: &App) -> gpui::Div {
    div()
        .truncate()
        .table_cell_size(prefs::table_cell_size(cx))
        .font_family(prefs::code_font_family(cx))
}

pub fn render_grid_cell(
    kind: ColumnValueKind,
    display: SharedString,
    is_null: bool,
    row_ix: usize,
    col_ix: usize,
    _window: &mut Window,
    cx: &mut App,
) -> impl IntoElement {
    let theme = cx.theme();
    if is_null {
        return cell_chrome(cx)
            .text_color(theme.muted_foreground)
            .child(display)
            .into_any_element();
    }

    match kind {
        ColumnValueKind::Numeric => cell_chrome(cx)
            .w_full()
            .text_right()
            .text_color(theme.blue_light)
            .child(display)
            .into_any_element(),
        ColumnValueKind::Boolean => {
            let label = display.to_string();
            if let Some(value) = parse_bool_display(&label) {
                let icon = if value {
                    Icon::new(IconName::CircleCheck)
                        .xsmall()
                        .text_color(theme.green_light)
                } else {
                    Icon::new(IconName::CircleX)
                        .xsmall()
                        .text_color(theme.muted_foreground)
                };
                let tooltip_label = label;
                let cell_id = row_ix.saturating_mul(10_000).saturating_add(col_ix);
                h_flex()
                    .id(("grid-cell-bool", cell_id))
                    .w_full()
                    .h_full()
                    .items_center()
                    .justify_center()
                    .child(icon)
                    .hoverable_tooltip(move |w, app| {
                        Tooltip::new(tooltip_label.clone()).build(w, app)
                    })
                    .into_any_element()
            } else {
                cell_chrome(cx)
                    .text_color(theme.foreground)
                    .child(display)
                    .into_any_element()
            }
        }
        ColumnValueKind::Text | ColumnValueKind::Unknown => cell_chrome(cx)
            .text_color(theme.foreground)
            .child(display)
            .into_any_element(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn normalize_strips_precision() {
        assert_eq!(normalize_type_name("numeric(10,2)"), "numeric");
        assert_eq!(
            normalize_type_name("character varying(255)"),
            "character varying"
        );
        assert_eq!(normalize_type_name("INT4"), "int4");
    }

    #[test]
    fn numeric_types_from_catalog_and_sqlx() {
        for ty in [
            "integer",
            "INT4",
            "numeric(10,2)",
            "double precision",
            "REAL",
            "money",
        ] {
            assert_eq!(
                column_value_kind(Some(ty)),
                ColumnValueKind::Numeric,
                "expected Numeric for {ty}"
            );
        }
    }

    #[test]
    fn boolean_types() {
        assert_eq!(column_value_kind(Some("bool")), ColumnValueKind::Boolean);
        assert_eq!(column_value_kind(Some("BOOL")), ColumnValueKind::Boolean);
    }

    #[test]
    fn text_types_not_numeric() {
        for ty in ["text", "varchar", "character varying(100)"] {
            assert_eq!(
                column_value_kind(Some(ty)),
                ColumnValueKind::Text,
                "expected Text for {ty}"
            );
        }
    }

    #[test]
    fn character_varying_does_not_match_int() {
        assert_eq!(
            column_value_kind(Some("character varying")),
            ColumnValueKind::Text
        );
    }

    #[test]
    fn missing_type_is_unknown() {
        assert_eq!(column_value_kind(None), ColumnValueKind::Unknown);
        assert_eq!(column_value_kind(Some("")), ColumnValueKind::Unknown);
    }

    #[test]
    fn parse_bool_display_values() {
        assert_eq!(parse_bool_display("true"), Some(true));
        assert_eq!(parse_bool_display("FALSE"), Some(false));
        assert_eq!(parse_bool_display("t"), Some(true));
        assert_eq!(parse_bool_display("f"), Some(false));
        assert_eq!(parse_bool_display("maybe"), None);
    }
}
