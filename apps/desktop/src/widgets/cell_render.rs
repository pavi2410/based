//! Schema-driven grid cell styling (numeric color, boolean icons).

use gpui_kit::component::{ActiveTheme, StyleSized, h_flex, tooltip::Tooltip};
use gpui_kit::{App, Div, IntoElement, SharedString, Window, div, prelude::*, px};

use based_core::Value;

use crate::app::prefs;
use crate::widgets::cell_hover::{self, HoverContent};
use crate::widgets::datetime;

/// Max chars of a text cell shown in its hover tooltip.
const TEXT_TOOLTIP_MAX: usize = 2_000;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ColumnValueKind {
    Numeric,
    Boolean,
    DateTime,
    Json,
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

    // Arrays render as Postgres literals (`{1,2}`); show them as text.
    if raw.trim().ends_with("[]") || raw.trim().eq_ignore_ascii_case("array") {
        return ColumnValueKind::Text;
    }

    let base = normalize_type_name(raw);
    // Datetime before numeric: sqlx-sqlite maps any declared type whose name
    // contains "int" (including `timestamptz`) to INTEGER.
    if is_datetime_type(&base) {
        return ColumnValueKind::DateTime;
    }
    if matches!(base.as_str(), "json" | "jsonb") {
        return ColumnValueKind::Json;
    }
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

/// Kind for one grid cell: the column's schema type when known, else the
/// value's own type (Mongo fields have no schema).
pub fn grid_cell_kind(value: &Value, data_type: Option<&str>, cell: &str) -> ColumnValueKind {
    match cell_value_kind(data_type, cell) {
        ColumnValueKind::Unknown => value_kind(value),
        kind => kind,
    }
}

fn value_kind(value: &Value) -> ColumnValueKind {
    match value {
        Value::Int(_) | Value::Float(_) | Value::Numeric(_) => ColumnValueKind::Numeric,
        Value::Bool(_) => ColumnValueKind::Boolean,
        Value::Date(_) | Value::Time(_) | Value::Timestamp(_) | Value::TimestampTz(_) => {
            ColumnValueKind::DateTime
        }
        Value::Json(_) => ColumnValueKind::Json,
        Value::Text(_) | Value::Uuid(_) => ColumnValueKind::Text,
        _ => ColumnValueKind::Unknown,
    }
}

/// Kind for one grid cell. Recovers `DateTime` when sqlx remapped `timestamptz`
/// (and similar) to `INTEGER` but the cell is still a parseable instant.
pub fn cell_value_kind(data_type: Option<&str>, cell: &str) -> ColumnValueKind {
    let kind = column_value_kind(data_type);
    if matches!(kind, ColumnValueKind::DateTime) || is_date_or_time_only(data_type) {
        return kind;
    }
    if datetime::parse_datetime_cell(cell).is_some() {
        return ColumnValueKind::DateTime;
    }
    kind
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

fn is_datetime_type(base: &str) -> bool {
    let compact = base.replace([' ', '_'], "");
    compact.contains("timestamp") || compact.contains("datetime")
}

fn is_date_or_time_only(data_type: Option<&str>) -> bool {
    let Some(raw) = data_type else {
        return false;
    };
    matches!(
        normalize_type_name(raw).as_str(),
        "date" | "time" | "time without time zone" | "time with time zone"
    )
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
            | "time"
            | "time without time zone"
            | "time with time zone"
    )
}

/// Format a numeric cell for display with thousands separators (display-only).
pub fn format_numeric_display(s: &str) -> String {
    let t = s.trim();
    if let Ok(n) = t.parse::<i64>() {
        return format_signed_integer(n);
    }
    if let Some(formatted) = format_decimal_string(t) {
        return formatted;
    }
    t.to_string()
}

fn format_signed_integer(n: i64) -> String {
    let negative = n < 0;
    let digits = n.unsigned_abs().to_string();
    let formatted = add_thousand_separators(&digits);
    if negative {
        format!("-{formatted}")
    } else {
        formatted
    }
}

fn format_decimal_string(s: &str) -> Option<String> {
    let (negative, rest) = strip_leading_sign(s.trim());
    let (int_part, frac_part) = rest
        .split_once('.')
        .map_or((rest, None), |(i, f)| (i, Some(f)));
    if int_part.is_empty() && frac_part.is_none() {
        return None;
    }
    if !int_part.is_empty() && !int_part.chars().all(|c| c.is_ascii_digit()) {
        return None;
    }
    if let Some(frac) = frac_part
        && !frac.is_empty()
        && !frac.chars().all(|c| c.is_ascii_digit())
    {
        return None;
    }

    let formatted_int = if int_part.is_empty() {
        "0".to_string()
    } else {
        add_thousand_separators(int_part)
    };

    let mut out = String::new();
    if negative {
        out.push('-');
    }
    out.push_str(&formatted_int);
    if let Some(frac) = frac_part {
        out.push('.');
        out.push_str(frac);
    }
    Some(out)
}

fn strip_leading_sign(s: &str) -> (bool, &str) {
    if let Some(rest) = s.strip_prefix('-') {
        (true, rest)
    } else if let Some(rest) = s.strip_prefix('+') {
        (false, rest)
    } else {
        (false, s)
    }
}

fn add_thousand_separators(digits: &str) -> String {
    if digits.is_empty() {
        return String::new();
    }
    let len = digits.len();
    let sep_count = (len - 1) / 3;
    let mut out = String::with_capacity(len + sep_count);
    for (i, ch) in digits.chars().enumerate() {
        if i > 0 && (len - i).is_multiple_of(3) {
            out.push(',');
        }
        out.push(ch);
    }
    out
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

fn cell_chrome(cx: &App) -> Div {
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
        // A dashed chip cannot be mistaken for the text "NULL".
        let muted = theme.muted_foreground;
        return h_flex()
            .h_full()
            .items_center()
            .child(
                div()
                    .px(px(5.0))
                    .rounded(px(4.0))
                    .border_1()
                    .border_dashed()
                    .border_color(muted.opacity(0.35))
                    .text_xs()
                    .italic()
                    .text_color(muted.opacity(0.8))
                    .font_family(prefs::code_font_family(cx))
                    .child(if display.trim().is_empty() {
                        SharedString::from("NULL")
                    } else {
                        display
                    }),
            )
            .into_any_element();
    }

    match kind {
        ColumnValueKind::Numeric => {
            let raw = display.to_string();
            let formatted = format_numeric_display(&raw);
            let show_raw_tooltip = formatted != raw;
            let cell_id = row_ix.saturating_mul(10_000).saturating_add(col_ix);
            let mut cell = cell_chrome(cx)
                .id(("grid-cell-num", cell_id))
                .w_full()
                .text_right()
                .text_color(theme.blue_light)
                .child(formatted);
            if show_raw_tooltip {
                let tooltip_raw = raw;
                cell = cell.hoverable_tooltip(move |w, app| {
                    Tooltip::new(tooltip_raw.clone()).build(w, app)
                });
            }
            cell.into_any_element()
        }
        ColumnValueKind::Boolean => {
            let label = display.to_string();
            if let Some(value) = parse_bool_display(&label) {
                let dot = if value {
                    theme.green_light
                } else {
                    theme.muted_foreground.opacity(0.6)
                };
                let tooltip_label = label;
                let cell_id = row_ix.saturating_mul(10_000).saturating_add(col_ix);
                h_flex()
                    .id(("grid-cell-bool", cell_id))
                    .w_full()
                    .h_full()
                    .gap(px(6.0))
                    .items_center()
                    .child(div().size(px(7.0)).rounded_full().bg(dot))
                    .child(
                        div()
                            .text_color(if value {
                                theme.foreground
                            } else {
                                theme.muted_foreground
                            })
                            .child(if value { "true" } else { "false" }),
                    )
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
        ColumnValueKind::DateTime => {
            let raw = display.to_string();
            let cell_id = row_ix.saturating_mul(10_000).saturating_add(col_ix);
            let cell = cell_chrome(cx)
                .id(("grid-cell-dt", cell_id))
                .w_full()
                .text_color(theme.foreground)
                .child(display);
            let content = HoverContent::Datetime(raw);
            if content.available(cx) {
                cell_hover::track(cell, cell_id as u64, content)
            } else {
                cell.into_any_element()
            }
        }
        ColumnValueKind::Json => {
            let cell_id = row_ix.saturating_mul(10_000).saturating_add(col_ix);
            let raw = display.to_string();
            let cell = cell_chrome(cx)
                .id(("grid-cell-json", cell_id))
                .w_full()
                .text_color(theme.foreground)
                .child(display);
            let content = HoverContent::Json(raw);
            if content.available(cx) {
                cell_hover::track(cell, cell_id as u64, content)
            } else {
                cell.into_any_element()
            }
        }
        ColumnValueKind::Text => {
            let cell_id = row_ix.saturating_mul(10_000).saturating_add(col_ix);
            let full: String = display.chars().take(TEXT_TOOLTIP_MAX).collect();
            cell_chrome(cx)
                .id(("grid-cell-text", cell_id))
                .w_full()
                .text_color(theme.foreground)
                .child(display)
                .hoverable_tooltip(move |w, app| {
                    let text = full.clone();
                    Tooltip::element(move |_, _| {
                        div()
                            .max_w(px(480.0))
                            .child(text.clone())
                            .into_any_element()
                    })
                    .build(w, app)
                })
                .into_any_element()
        }
        ColumnValueKind::Unknown => cell_chrome(cx)
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
    fn datetime_types() {
        for ty in [
            "timestamp",
            "timestamptz",
            "DATETIME",
            "timestamp without time zone",
            "timestamp with time zone(6)",
        ] {
            assert_eq!(
                column_value_kind(Some(ty)),
                ColumnValueKind::DateTime,
                "expected DateTime for {ty}"
            );
        }
        assert_eq!(column_value_kind(Some("date")), ColumnValueKind::Text);
        assert_eq!(column_value_kind(Some("time")), ColumnValueKind::Text);
        assert_eq!(
            column_value_kind(Some("time with time zone")),
            ColumnValueKind::Text
        );
    }

    #[test]
    fn sqlx_sqlite_timestamptz_remap_recovers_from_cell() {
        // sqlx-sqlite: `_ if s.contains("int") => Integer`, so TIMESTAMPTZ → INTEGER.
        assert_eq!(column_value_kind(Some("INTEGER")), ColumnValueKind::Numeric);
        assert_eq!(
            cell_value_kind(Some("INTEGER"), "2020-01-15 12:30:00+00"),
            ColumnValueKind::DateTime
        );
        assert_eq!(
            cell_value_kind(Some("INTEGER"), "42"),
            ColumnValueKind::Numeric
        );
        assert_eq!(
            cell_value_kind(Some("timestamptz"), "2020-01-15 12:30:00+00"),
            ColumnValueKind::DateTime
        );
        assert_eq!(
            cell_value_kind(Some("timestamp with time zone"), "2020-01-15 12:30:00+00"),
            ColumnValueKind::DateTime
        );
    }

    #[test]
    fn date_and_time_only_cells_stay_plain() {
        assert_eq!(
            cell_value_kind(Some("date"), "2020-01-15"),
            ColumnValueKind::Text
        );
        assert_eq!(
            cell_value_kind(Some("time"), "12:30:00"),
            ColumnValueKind::Text
        );
        assert_eq!(
            cell_value_kind(Some("DATE"), "2020-01-15"),
            ColumnValueKind::Text
        );
        assert_eq!(
            cell_value_kind(Some("INTEGER"), "2020-01-15"),
            ColumnValueKind::Numeric
        );
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

    #[test]
    fn format_numeric_display_adds_separators() {
        assert_eq!(format_numeric_display("1234567"), "1,234,567");
        assert_eq!(format_numeric_display("-1234"), "-1,234");
        assert_eq!(format_numeric_display("1234.56"), "1,234.56");
        assert_eq!(format_numeric_display("3.14"), "3.14");
        assert_eq!(format_numeric_display("not-a-number"), "not-a-number");
    }
}
