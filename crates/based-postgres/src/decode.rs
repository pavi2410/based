//! Shared Postgres cell decoder for the query grid and the data viewer.

use sqlx::postgres::{PgRow, PgValueFormat};
use sqlx::{Row, TypeInfo, ValueRef};
use time::{Date, Duration, Month, PrimitiveDateTime, Time};

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
/// `None` is SQL NULL. Query execution and the data viewer both go through here.
pub fn decode_pg_cell(type_name: &str, bytes: Option<&[u8]>) -> String {
    let Some(bytes) = bytes else {
        return NULL_CELL.into();
    };
    decode_binary(normalize_pg_type(type_name), bytes).unwrap_or_else(|| NULL_CELL.into())
}

fn normalize_pg_type(name: &str) -> &str {
    let name = name.trim();
    match name {
        "int2" | "INT2" | "smallint" | "SMALLINT" | "smallserial" | "SMALLSERIAL" => "INT2",
        "int4" | "INT4" | "int" | "INT" | "integer" | "INTEGER" | "serial" | "SERIAL" => "INT4",
        "int8" | "INT8" | "bigint" | "BIGINT" | "bigserial" | "BIGSERIAL" => "INT8",
        "float4" | "FLOAT4" | "real" | "REAL" => "FLOAT4",
        "float8" | "FLOAT8" | "float" | "FLOAT" | "double precision" | "DOUBLE PRECISION" => {
            "FLOAT8"
        }
        "bool" | "BOOL" | "boolean" | "BOOLEAN" => "BOOL",
        "text" | "TEXT" | "varchar" | "VARCHAR" | "bpchar" | "BPCHAR" | "name" | "NAME"
        | "char" | "CHAR" | "character" | "CHARACTER" | "character varying"
        | "CHARACTER VARYING" | "citext" | "CITEXT" => "TEXT",
        "bytea" | "BYTEA" => "BYTEA",
        "date" | "DATE" => "DATE",
        "time" | "TIME" | "time without time zone" | "TIME WITHOUT TIME ZONE" => "TIME",
        "timestamp"
        | "TIMESTAMP"
        | "timestamp without time zone"
        | "TIMESTAMP WITHOUT TIME ZONE" => "TIMESTAMP",
        "timestamptz" | "TIMESTAMPTZ" | "timestamp with time zone" | "TIMESTAMP WITH TIME ZONE" => {
            "TIMESTAMPTZ"
        }
        "numeric" | "NUMERIC" | "decimal" | "DECIMAL" => "NUMERIC",
        "json" | "JSON" => "JSON",
        "jsonb" | "JSONB" => "JSONB",
        "uuid" | "UUID" => "UUID",
        other => other,
    }
}

fn decode_binary(type_name: &str, bytes: &[u8]) -> Option<String> {
    match type_name {
        "INT2" => Some(i16::from_be_bytes(bytes.try_into().ok()?).to_string()),
        "INT4" => Some(i32::from_be_bytes(bytes.try_into().ok()?).to_string()),
        "INT8" => Some(i64::from_be_bytes(bytes.try_into().ok()?).to_string()),
        "FLOAT4" => Some(f32::from_be_bytes(bytes.try_into().ok()?).to_string()),
        "FLOAT8" => Some(f64::from_be_bytes(bytes.try_into().ok()?).to_string()),
        "BOOL" => Some((bytes.first().copied()? != 0).to_string()),
        "TEXT" | "JSON" => String::from_utf8(bytes.to_vec()).ok(),
        "JSONB" => decode_jsonb(bytes),
        "BYTEA" => Some(format!("<{} bytes>", bytes.len())),
        "DATE" => decode_date(bytes),
        "TIME" => decode_time(bytes),
        "TIMESTAMP" => decode_timestamp(bytes, false),
        "TIMESTAMPTZ" => decode_timestamp(bytes, true),
        "NUMERIC" => decode_numeric(bytes),
        "UUID" => decode_uuid(bytes),
        _ => String::from_utf8(bytes.to_vec()).ok(),
    }
}

fn decode_jsonb(bytes: &[u8]) -> Option<String> {
    let rest = match bytes.first().copied() {
        Some(1) => bytes.get(1..)?,
        _ => bytes,
    };
    String::from_utf8(rest.to_vec()).ok()
}

fn decode_uuid(bytes: &[u8]) -> Option<String> {
    if bytes.len() != 16 {
        return None;
    }
    Some(format!(
        "{:02x}{:02x}{:02x}{:02x}-{:02x}{:02x}-{:02x}{:02x}-{:02x}{:02x}-{:02x}{:02x}{:02x}{:02x}{:02x}{:02x}",
        bytes[0],
        bytes[1],
        bytes[2],
        bytes[3],
        bytes[4],
        bytes[5],
        bytes[6],
        bytes[7],
        bytes[8],
        bytes[9],
        bytes[10],
        bytes[11],
        bytes[12],
        bytes[13],
        bytes[14],
        bytes[15],
    ))
}

fn pg_epoch() -> Option<Date> {
    Date::from_calendar_date(2000, Month::January, 1).ok()
}

fn decode_date(bytes: &[u8]) -> Option<String> {
    let days = i32::from_be_bytes(bytes.try_into().ok()?);
    if days == i32::MAX {
        return Some("infinity".into());
    }
    if days == i32::MIN {
        return Some("-infinity".into());
    }
    let date = Date::from_julian_day(pg_epoch()?.to_julian_day().checked_add(days)?).ok()?;
    Some(format_date(date))
}

fn decode_time(bytes: &[u8]) -> Option<String> {
    let us = i64::from_be_bytes(bytes.try_into().ok()?);
    if us == 86_400_000_000 {
        return Some("24:00:00".into());
    }
    if !(0..86_400_000_000).contains(&us) {
        return None;
    }
    let secs = (us / 1_000_000) as u32;
    let micros = (us % 1_000_000) as u32;
    let time = Time::from_hms_micro(
        (secs / 3600) as u8,
        ((secs % 3600) / 60) as u8,
        (secs % 60) as u8,
        micros,
    )
    .ok()?;
    Some(format_clock(time))
}

fn decode_timestamp(bytes: &[u8], with_tz: bool) -> Option<String> {
    let us = i64::from_be_bytes(bytes.try_into().ok()?);
    if us == i64::MAX {
        return Some("infinity".into());
    }
    if us == i64::MIN {
        return Some("-infinity".into());
    }
    let dt = PrimitiveDateTime::new(pg_epoch()?, Time::MIDNIGHT)
        .checked_add(Duration::microseconds(us))?;
    let mut s = format!("{} {}", format_date(dt.date()), format_clock(dt.time()));
    if with_tz {
        s.push_str("+00");
    }
    Some(s)
}

fn format_date(date: Date) -> String {
    format!(
        "{:04}-{:02}-{:02}",
        date.year(),
        date.month() as u8,
        date.day()
    )
}

fn format_clock(time: Time) -> String {
    let mut s = format!(
        "{:02}:{:02}:{:02}",
        time.hour(),
        time.minute(),
        time.second()
    );
    if time.microsecond() != 0 {
        s.push_str(&format!(".{:06}", time.microsecond()));
        while s.ends_with('0') {
            s.pop();
        }
        if s.ends_with('.') {
            s.pop();
        }
    }
    s
}

const NUMERIC_POS: u16 = 0x0000;
const NUMERIC_NEG: u16 = 0x4000;
const NUMERIC_NAN: u16 = 0xC000;
const NUMERIC_PINF: u16 = 0xD000;
const NUMERIC_NINF: u16 = 0xF000;

fn read_i16(bytes: &[u8], off: usize) -> Option<i16> {
    Some(i16::from_be_bytes(
        bytes.get(off..off + 2)?.try_into().ok()?,
    ))
}

fn decode_numeric(bytes: &[u8]) -> Option<String> {
    let ndigits = read_i16(bytes, 0)? as usize;
    let weight = read_i16(bytes, 2)?;
    let sign = read_i16(bytes, 4)? as u16;
    let dscale = read_i16(bytes, 6)?;
    if bytes.len() < 8 + ndigits * 2 {
        return None;
    }
    let mut digits = Vec::with_capacity(ndigits);
    for i in 0..ndigits {
        digits.push(read_i16(bytes, 8 + i * 2)?);
    }

    match sign {
        NUMERIC_NAN => return Some("NaN".into()),
        NUMERIC_PINF => return Some("Infinity".into()),
        NUMERIC_NINF => return Some("-Infinity".into()),
        NUMERIC_POS | NUMERIC_NEG => {}
        _ => return None,
    }

    if ndigits == 0 {
        return Some(if dscale > 0 {
            format!(
                "{}0.{}",
                if sign == NUMERIC_NEG { "-" } else { "" },
                "0".repeat(dscale as usize)
            )
        } else if sign == NUMERIC_NEG {
            "-0".into()
        } else {
            "0".into()
        });
    }

    let mut s = String::new();
    if sign == NUMERIC_NEG {
        s.push('-');
    }

    let integer_groups = i32::from(weight) + 1;
    if integer_groups <= 0 {
        s.push('0');
    } else {
        for i in 0..integer_groups {
            let d = digits.get(i as usize).copied().unwrap_or(0);
            if i == 0 {
                s.push_str(&d.to_string());
            } else {
                s.push_str(&format!("{d:04}"));
            }
        }
    }

    if dscale > 0 {
        s.push('.');
        let mut remaining = dscale as usize;
        if weight < -1 {
            let leading = ((-1 - i32::from(weight)) as usize).saturating_mul(4);
            let zeros = leading.min(remaining);
            s.push_str(&"0".repeat(zeros));
            remaining -= zeros;
        }
        let mut idx = if weight >= 0 {
            (weight + 1) as usize
        } else {
            0
        };
        while remaining > 0 {
            let d = digits.get(idx).copied().unwrap_or(0);
            let chunk = format!("{d:04}");
            let take = remaining.min(4);
            s.push_str(&chunk[..take]);
            remaining -= take;
            idx += 1;
        }
    }

    Some(s)
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
        buf.extend_from_slice(&be_i16(2));
        buf.extend_from_slice(&be_i16(0));
        buf.extend_from_slice(&be_i16(0));
        buf.extend_from_slice(&be_i16(2));
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
        let ts_us: i64 =
            i64::from(date_days) * 86_400_000_000 + 12 * 3_600_000_000 + 30 * 60_000_000;
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
