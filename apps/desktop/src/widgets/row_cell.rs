//! Format sqlx row values as display strings for data grids.

use sqlx::{Row, sqlite::SqliteRow, postgres::PgRow};

use super::virtual_table::NULL_CELL_DISPLAY;

const NULL_DISPLAY: &str = NULL_CELL_DISPLAY;

fn format_optional<T: std::fmt::Display>(v: Option<T>) -> String {
    v.map(|x| x.to_string())
        .unwrap_or_else(|| NULL_DISPLAY.into())
}

/// Format a column from a SQLite row.
pub fn sqlite_cell_display(row: &SqliteRow, col: usize) -> String {
    if let Ok(v) = row.try_get::<Option<i64>, _>(col) {
        return format_optional(v);
    }
    if let Ok(v) = row.try_get::<Option<f64>, _>(col) {
        return format_optional(v);
    }
    if let Ok(v) = row.try_get::<Option<bool>, _>(col) {
        return format_optional(v);
    }
    if let Ok(v) = row.try_get::<Option<String>, _>(col) {
        return v.unwrap_or_else(|| NULL_DISPLAY.into());
    }
    if let Ok(v) = row.try_get::<Option<Vec<u8>>, _>(col) {
        return v.map(|b| format!("<{} bytes>", b.len()))
            .unwrap_or_else(|| NULL_DISPLAY.into());
    }
    NULL_DISPLAY.into()
}

/// Format a column from a Postgres row.
pub fn pg_cell_display(row: &PgRow, col: usize) -> String {
    if let Ok(v) = row.try_get::<Option<i64>, _>(col) {
        return format_optional(v);
    }
    if let Ok(v) = row.try_get::<Option<i32>, _>(col) {
        return format_optional(v);
    }
    if let Ok(v) = row.try_get::<Option<f64>, _>(col) {
        return format_optional(v);
    }
    if let Ok(v) = row.try_get::<Option<bool>, _>(col) {
        return format_optional(v);
    }
    if let Ok(v) = row.try_get::<Option<String>, _>(col) {
        return v.unwrap_or_else(|| NULL_DISPLAY.into());
    }
    if let Ok(v) = row.try_get::<Option<Vec<u8>>, _>(col) {
        return v.map(|b| format!("<{} bytes>", b.len()))
            .unwrap_or_else(|| NULL_DISPLAY.into());
    }
    NULL_DISPLAY.into()
}
