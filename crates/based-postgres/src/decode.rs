//! Shared Postgres cell decoder for the query grid and the data viewer.

use sqlx::postgres::{PgRow, PgValueFormat};
use sqlx::{Row, TypeInfo, ValueRef};

pub const NULL_CELL: &str = "NULL";

/// Format a column from a live `PgRow` for grid display.
pub fn pg_cell_display(row: &PgRow, col: usize) -> String {
    let Ok(raw) = row.try_get_raw(col) else {
        return NULL_CELL.into();
    };
    if raw.is_null() {
        return NULL_CELL.into();
    }
    let type_info = raw.type_info();
    let type_name = type_info.name();
    match raw.format() {
        PgValueFormat::Text => raw
            .as_str()
            .map(str::to_string)
            .unwrap_or_else(|_| NULL_CELL.into()),
        PgValueFormat::Binary => match raw.as_bytes() {
            Ok(bytes) => decode_pg_cell(type_name, Some(bytes)),
            Err(_) => NULL_CELL.into(),
        },
    }
}

/// Decode one Postgres cell from its type name and optional binary payload.
///
/// `None` is SQL NULL. This is the single decoder used by `execute_sql` and
/// the data viewer (`pg_cell_display`).
pub fn decode_pg_cell(_type_name: &str, bytes: Option<&[u8]>) -> String {
    // Matches the current query path: a failed `try_get::<String>` becomes "".
    let _ = bytes;
    String::new()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn be_i16(v: i16) -> [u8; 2] {
        v.to_be_bytes()
    }

    fn be_i32(v: i32) -> [u8; 4] {
        v.to_be_bytes()
    }

    fn be_i64(v: i64) -> [u8; 8] {
        v.to_be_bytes()
    }

    fn numeric_123_45() -> Vec<u8> {
        let mut buf = Vec::new();
        buf.extend_from_slice(&be_i16(2)); // ndigits
        buf.extend_from_slice(&be_i16(0)); // weight
        buf.extend_from_slice(&be_i16(0)); // sign +
        buf.extend_from_slice(&be_i16(2)); // dscale
        buf.extend_from_slice(&be_i16(123));
        buf.extend_from_slice(&be_i16(4500));
        buf
    }

    /// Binary fixtures for the types that currently render as blank cells.
    fn fixtures() -> Vec<(&'static str, Option<Vec<u8>>, &'static str)> {
        let uuid = [
            0x55, 0x0e, 0x84, 0x00, 0xe2, 0x9b, 0x41, 0xd4, 0xa7, 0x16, 0x44, 0x66, 0x55, 0x44,
            0x00, 0x00,
        ];
        // 2020-01-15 is 7319 days after 2000-01-01.
        let date_days: i32 = 7319;
        // 2020-01-15 12:30:00 = 7319 days + 12h30m, in microseconds.
        let ts_us: i64 = i64::from(date_days) * 86_400_000_000 + 12 * 3_600_000_000 + 30 * 60_000_000;
        vec![
            ("INT2", Some(be_i16(42).to_vec()), "42"),
            ("INT4", Some(be_i32(42).to_vec()), "42"),
            ("INT8", Some(be_i64(42).to_vec()), "42"),
            ("FLOAT4", Some(1.5f32.to_be_bytes().to_vec()), "1.5"),
            ("FLOAT8", Some(1.5f64.to_be_bytes().to_vec()), "1.5"),
            ("BOOL", Some(vec![1]), "true"),
            ("TEXT", Some(b"hello".to_vec()), "hello"),
            ("BYTEA", Some(vec![0xde, 0xad, 0xbe, 0xef]), "<4 bytes>"),
            ("DATE", Some(be_i32(date_days).to_vec()), "2020-01-15"),
            (
                "TIMESTAMP",
                Some(be_i64(ts_us).to_vec()),
                "2020-01-15 12:30:00",
            ),
            (
                "TIMESTAMPTZ",
                Some(be_i64(ts_us).to_vec()),
                "2020-01-15 12:30:00+00",
            ),
            ("NUMERIC", Some(numeric_123_45()), "123.45"),
            ("JSON", Some(br#"{"a":1}"#.to_vec()), r#"{"a":1}"#),
            (
                "JSONB",
                Some({
                    let mut v = vec![1];
                    v.extend_from_slice(br#"{"a":1}"#);
                    v
                }),
                r#"{"a":1}"#,
            ),
            (
                "UUID",
                Some(uuid.to_vec()),
                "550e8400-e29b-41d4-a716-446655440000",
            ),
            ("TEXT", None, NULL_CELL),
        ]
    }

    #[test]
    fn decode_pg_cell_shows_values_not_blanks() {
        for (type_name, bytes, expected) in fixtures() {
            let got = decode_pg_cell(type_name, bytes.as_deref());
            assert_eq!(
                got, expected,
                "{type_name}: expected {expected:?}, got {got:?}"
            );
            assert!(
                !got.is_empty() || expected.is_empty(),
                "{type_name} rendered as a blank cell"
            );
        }
    }
}
