//! Shared Postgres cell decoder for the query grid and the data viewer.

use std::str;
use std::sync::Arc;

use based_core::Value;
use sqlx::postgres::{PgRow, PgValueFormat};
use sqlx::{Row, TypeInfo, ValueRef};
use time::{Date, Duration, Month, PrimitiveDateTime, Time};

/// Decode a column from a live `PgRow`.
pub fn pg_cell(row: &PgRow, col: usize) -> Value {
    let Ok(raw) = row.try_get_raw(col) else {
        return Value::undecodable("unknown");
    };
    if raw.is_null() {
        return Value::Null;
    }
    let type_info = raw.type_info();
    let type_name = type_info.name();
    match raw.format() {
        PgValueFormat::Text => match raw.as_str() {
            Ok(s) => parse_pg_text(type_name, s),
            Err(_) => Value::undecodable(type_name),
        },
        PgValueFormat::Binary => match raw.as_bytes() {
            Ok(bytes) => decode_pg_cell(type_name, Some(bytes)),
            Err(_) => Value::undecodable(type_name),
        },
    }
}

/// Decode one Postgres cell from its type name and optional binary payload.
///
/// `None` is SQL NULL. Bytes the decoder can't read become [`Value::Other`],
/// never NULL.
pub fn decode_pg_cell(type_name: &str, bytes: Option<&[u8]>) -> Value {
    let Some(bytes) = bytes else {
        return Value::Null;
    };
    let type_name = type_name.trim();
    let decoded = match array_elem_type(type_name) {
        Some(elem) => decode_array(normalize_pg_type(elem), bytes),
        None => decode_binary(normalize_pg_type(type_name), bytes),
    };
    decoded.unwrap_or_else(|| Value::undecodable(type_name))
}

/// Type a value Postgres sent in text format. Types without a text parser stay text.
fn parse_pg_text(type_name: &str, s: &str) -> Value {
    let parsed = match normalize_pg_type(type_name.trim()) {
        "INT2" | "INT4" | "INT8" => s.parse().ok().map(Value::Int),
        "FLOAT4" | "FLOAT8" => s.parse().ok().map(Value::Float),
        "BOOL" => match s {
            "t" | "true" => Some(Value::Bool(true)),
            "f" | "false" => Some(Value::Bool(false)),
            _ => None,
        },
        "NUMERIC" => Some(Value::Numeric(s.into())),
        "JSON" | "JSONB" => Some(Value::Json(s.into())),
        _ => None,
    };
    parsed.unwrap_or_else(|| Value::text(s))
}

/// Element type name for array types (`INT4[]`, `_text`); `None` for scalars.
fn array_elem_type(name: &str) -> Option<&str> {
    name.strip_suffix("[]")
        .or_else(|| name.strip_prefix('_').filter(|n| !n.is_empty()))
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

fn decode_binary(type_name: &str, bytes: &[u8]) -> Option<Value> {
    match type_name {
        "INT2" => Some(Value::Int(
            i16::from_be_bytes(bytes.try_into().ok()?).into(),
        )),
        "INT4" => Some(Value::Int(
            i32::from_be_bytes(bytes.try_into().ok()?).into(),
        )),
        "INT8" => Some(Value::Int(i64::from_be_bytes(bytes.try_into().ok()?))),
        // Through the f32's shortest text so 0.1::real reads 0.1, not 0.10000000149.
        "FLOAT4" => f32::from_be_bytes(bytes.try_into().ok()?)
            .to_string()
            .parse()
            .ok()
            .map(Value::Float),
        "FLOAT8" => Some(Value::Float(f64::from_be_bytes(bytes.try_into().ok()?))),
        "BOOL" => Some(Value::Bool(bytes.first().copied()? != 0)),
        "TEXT" => utf8(bytes).map(Value::Text),
        "JSON" => utf8(bytes).map(Value::Json),
        "JSONB" => decode_jsonb(bytes),
        "BYTEA" => Some(Value::Bytes(bytes.into())),
        "DATE" => decode_date(bytes),
        "TIME" => decode_time(bytes),
        "TIMESTAMP" => decode_timestamp(bytes, false),
        "TIMESTAMPTZ" => decode_timestamp(bytes, true),
        "NUMERIC" => decode_numeric(bytes).map(|s| Value::Numeric(s.into())),
        "UUID" => Some(Value::Uuid(bytes.try_into().ok()?)),
        // Binary layouts we don't parse: reading them as UTF-8 would show garbage.
        other if is_binary_layout(other) => None,
        // Enums, domains over text, `xml`, `citext` from an extension …: their
        // binary form is the text itself.
        _ => utf8(bytes)
            .filter(|s| {
                !s.chars()
                    .any(|c| c.is_control() && !matches!(c, '\n' | '\r' | '\t'))
            })
            .map(Value::Text),
    }
}

/// Built-in types whose binary wire format is not UTF-8 text.
fn is_binary_layout(type_name: &str) -> bool {
    let t = type_name.to_ascii_uppercase();
    matches!(
        t.as_str(),
        "INTERVAL"
            | "TIMETZ"
            | "MONEY"
            | "INET"
            | "CIDR"
            | "MACADDR"
            | "MACADDR8"
            | "OID"
            | "XID"
            | "XID8"
            | "CID"
            | "TID"
            | "PG_LSN"
            | "BIT"
            | "VARBIT"
            | "POINT"
            | "LINE"
            | "LSEG"
            | "BOX"
            | "PATH"
            | "POLYGON"
            | "CIRCLE"
            | "TSVECTOR"
            | "TSQUERY"
    ) || t.ends_with("RANGE")
}

fn utf8(bytes: &[u8]) -> Option<Arc<str>> {
    str::from_utf8(bytes).ok().map(Arc::from)
}

fn special(type_name: &str, text: &str) -> Value {
    Value::Other {
        type_name: type_name.into(),
        text: Some(text.into()),
    }
}

fn read_i32(bytes: &[u8], off: usize) -> Option<i32> {
    Some(i32::from_be_bytes(
        bytes.get(off..off + 4)?.try_into().ok()?,
    ))
}

/// Decode a binary array (nested for N-d).
fn decode_array(elem: &str, bytes: &[u8]) -> Option<Value> {
    let ndim = usize::try_from(read_i32(bytes, 0)?).ok()?;
    // bytes 4..8 flags, 8..12 element OID.
    let mut off = 12;
    let mut dims = Vec::with_capacity(ndim);
    for _ in 0..ndim {
        dims.push(usize::try_from(read_i32(bytes, off)?).ok()?);
        off += 8; // length + lower bound
    }
    if ndim == 0 {
        return Some(Value::Array(Arc::from([])));
    }
    read_array_level(elem, bytes, &mut off, &dims)
}

fn read_array_level(elem: &str, bytes: &[u8], off: &mut usize, dims: &[usize]) -> Option<Value> {
    let mut items = Vec::with_capacity(dims[0]);
    for _ in 0..dims[0] {
        if dims.len() > 1 {
            items.push(read_array_level(elem, bytes, off, &dims[1..])?);
            continue;
        }
        let len = read_i32(bytes, *off)?;
        *off += 4;
        if len < 0 {
            items.push(Value::Null);
            continue;
        }
        let end = off.checked_add(len as usize)?;
        items.push(decode_binary(elem, bytes.get(*off..end)?)?);
        *off = end;
    }
    Some(Value::Array(items.into()))
}

fn decode_jsonb(bytes: &[u8]) -> Option<Value> {
    let rest = match bytes.first().copied() {
        Some(1) => bytes.get(1..)?,
        _ => bytes,
    };
    utf8(rest).map(Value::Json)
}

fn pg_epoch() -> Option<Date> {
    Date::from_calendar_date(2000, Month::January, 1).ok()
}

fn decode_date(bytes: &[u8]) -> Option<Value> {
    let days = i32::from_be_bytes(bytes.try_into().ok()?);
    if days == i32::MAX {
        return Some(special("DATE", "infinity"));
    }
    if days == i32::MIN {
        return Some(special("DATE", "-infinity"));
    }
    let date = Date::from_julian_day(pg_epoch()?.to_julian_day().checked_add(days)?).ok()?;
    Some(Value::Date(date))
}

fn decode_time(bytes: &[u8]) -> Option<Value> {
    let us = i64::from_be_bytes(bytes.try_into().ok()?);
    if us == 86_400_000_000 {
        return Some(special("TIME", "24:00:00"));
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
    Some(Value::Time(time))
}

/// `timestamptz` arrives as UTC microseconds.
fn decode_timestamp(bytes: &[u8], with_tz: bool) -> Option<Value> {
    let type_name = if with_tz { "TIMESTAMPTZ" } else { "TIMESTAMP" };
    let us = i64::from_be_bytes(bytes.try_into().ok()?);
    if us == i64::MAX {
        return Some(special(type_name, "infinity"));
    }
    if us == i64::MIN {
        return Some(special(type_name, "-infinity"));
    }
    let dt = PrimitiveDateTime::new(pg_epoch()?, Time::MIDNIGHT)
        .checked_add(Duration::microseconds(us))?;
    Some(if with_tz {
        Value::TimestampTz(dt.assume_utc())
    } else {
        Value::Timestamp(dt)
    })
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

    /// Binary fixtures and their grid text.
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
            ("FLOAT4", Some(0.1f32.to_be_bytes().to_vec()), "0.1"),
            ("FLOAT8", Some(1.5f64.to_be_bytes().to_vec()), "1.5"),
            ("BOOL", Some(vec![1]), "true"),
            ("TEXT", Some(b"hello".to_vec()), "hello"),
            ("BYTEA", Some(vec![0xde, 0xad, 0xbe, 0xef]), "<4 bytes>"),
            ("DATE", Some(be_i32(date_days).to_vec()), "2020-01-15"),
            ("DATE", Some(be_i32(i32::MAX).to_vec()), "infinity"),
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
            ("TEXT", None, "NULL"),
            // A user-defined enum: binary form is the label.
            ("mood", Some(b"happy".to_vec()), "happy"),
        ]
    }

    fn array_bytes(elem_oid: i32, dims: &[i32], elems: &[Option<&[u8]>]) -> Vec<u8> {
        let mut b = Vec::new();
        b.extend_from_slice(&be_i32(dims.len() as i32));
        b.extend_from_slice(&be_i32(i32::from(elems.iter().any(Option::is_none))));
        b.extend_from_slice(&be_i32(elem_oid));
        for d in dims {
            b.extend_from_slice(&be_i32(*d));
            b.extend_from_slice(&be_i32(1));
        }
        for e in elems {
            match e {
                Some(v) => {
                    b.extend_from_slice(&be_i32(v.len() as i32));
                    b.extend_from_slice(v);
                }
                None => b.extend_from_slice(&be_i32(-1)),
            }
        }
        b
    }

    fn shown(type_name: &str, bytes: Option<&[u8]>) -> String {
        decode_pg_cell(type_name, bytes).display()
    }

    #[test]
    fn decodes_arrays() {
        let ints = array_bytes(23, &[3], &[Some(&be_i32(1)), None, Some(&be_i32(3))]);
        assert_eq!(
            decode_pg_cell("INT4[]", Some(&ints)),
            Value::Array([Value::Int(1), Value::Null, Value::Int(3)].into())
        );
        assert_eq!(shown("INT4[]", Some(&ints)), "{1,NULL,3}");
        let texts = array_bytes(25, &[2], &[Some(b"a b"), Some(b"c")]);
        assert_eq!(shown("TEXT[]", Some(&texts)), r#"{"a b",c}"#);
        let grid = array_bytes(
            23,
            &[2, 2],
            &[
                Some(&be_i32(1)),
                Some(&be_i32(2)),
                Some(&be_i32(3)),
                Some(&be_i32(4)),
            ],
        );
        assert_eq!(shown("_int4", Some(&grid)), "{{1,2},{3,4}}");
        assert_eq!(shown("INT4[]", Some(&be_i32(0).repeat(3))), "{}");
    }

    #[test]
    fn decode_pg_cell_shows_values_not_blanks() {
        for (type_name, bytes, expected) in fixtures() {
            let got = shown(type_name, bytes.as_deref());
            assert_eq!(
                got, expected,
                "{type_name}: expected {expected:?}, got {got:?}"
            );
        }
    }

    #[test]
    fn null_is_typed_and_text_null_is_text() {
        assert_eq!(decode_pg_cell("TEXT", None), Value::Null);
        assert_eq!(decode_pg_cell("TEXT", Some(b"NULL")), Value::text("NULL"));
        assert_eq!(decode_pg_cell("TEXT", Some(b"")), Value::text(""));
    }

    #[test]
    fn undecodable_is_not_null() {
        // A 16-byte binary INTERVAL: we don't parse it, but it isn't NULL.
        let got = decode_pg_cell("INTERVAL", Some(&[0u8; 16]));
        assert!(!got.is_null());
        assert_eq!(got.display(), "<interval>");
        // Unknown type whose bytes aren't text.
        assert_eq!(
            decode_pg_cell("mystery", Some(&[0u8, 1, 2])).display(),
            "<mystery>"
        );
        // Malformed bytes for a known type.
        assert_eq!(decode_pg_cell("INT4", Some(&[1, 2])).display(), "<int4>");
    }

    #[test]
    fn types_text_format() {
        assert_eq!(parse_pg_text("INT4", "7"), Value::Int(7));
        assert_eq!(parse_pg_text("BOOL", "t"), Value::Bool(true));
        assert_eq!(
            parse_pg_text("NUMERIC", "1.50"),
            Value::Numeric("1.50".into())
        );
        assert_eq!(parse_pg_text("INTERVAL", "1 day"), Value::text("1 day"));
    }
}
