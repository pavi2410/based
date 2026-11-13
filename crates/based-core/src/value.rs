//! Typed cell values shared by every engine and the result grids.
//!
//! Engines decode into [`Value`]; the UI formats it. SQL NULL is a variant, never
//! a display string, so a text value `'NULL'` or `''` stays text.

use std::fmt::Write as _;
use std::sync::Arc;

use time::{Date, OffsetDateTime, PrimitiveDateTime, Time, UtcOffset};

/// What a grid shows for SQL NULL.
pub const NULL_DISPLAY: &str = "NULL";
/// What a grid shows for a field a document doesn't have.
pub const MISSING_DISPLAY: &str = "—";

/// One decoded cell.
#[derive(Debug, Clone, PartialEq)]
pub enum Value {
    /// SQL NULL / BSON null.
    Null,
    /// The document has no such field (MongoDB).
    Missing,
    Bool(bool),
    Int(i64),
    Float(f64),
    /// Exact decimal text (`NUMERIC`, `Decimal128`), including `NaN`/`Infinity`.
    Numeric(Arc<str>),
    Text(Arc<str>),
    /// Raw JSON text.
    Json(Arc<str>),
    Date(Date),
    Time(Time),
    Timestamp(PrimitiveDateTime),
    TimestampTz(OffsetDateTime),
    Uuid([u8; 16]),
    Bytes(Arc<[u8]>),
    Array(Arc<[Value]>),
    /// A value the decoder can't type further. `text` is the engine's own
    /// rendering when it has one (`infinity`, `24:00:00`); `None` means the
    /// bytes could not be decoded at all.
    Other {
        type_name: Arc<str>,
        text: Option<Arc<str>>,
    },
}

impl Value {
    pub fn text(s: impl Into<Arc<str>>) -> Self {
        Self::Text(s.into())
    }

    /// A value the decoder could not read, labelled with its type.
    pub fn undecodable(type_name: impl Into<Arc<str>>) -> Self {
        Self::Other {
            type_name: type_name.into(),
            text: None,
        }
    }

    /// SQL NULL or an absent document field.
    pub fn is_null(&self) -> bool {
        matches!(self, Self::Null | Self::Missing)
    }

    /// Grid text for this value. Arrays use Postgres literal syntax (`{1,NULL,"a b"}`).
    pub fn display(&self) -> String {
        match self {
            Self::Null => NULL_DISPLAY.into(),
            Self::Missing => MISSING_DISPLAY.into(),
            Self::Bool(b) => b.to_string(),
            Self::Int(n) => n.to_string(),
            Self::Float(f) => f.to_string(),
            Self::Numeric(s) | Self::Text(s) | Self::Json(s) => s.to_string(),
            Self::Date(d) => format_date(*d),
            Self::Time(t) => format_clock(*t),
            Self::Timestamp(dt) => {
                format!("{} {}", format_date(dt.date()), format_clock(dt.time()))
            }
            Self::TimestampTz(dt) => format!(
                "{} {}{}",
                format_date(dt.date()),
                format_clock(dt.time()),
                format_offset(dt.offset())
            ),
            Self::Uuid(b) => format_uuid(b),
            Self::Bytes(b) => format!("<{} bytes>", b.len()),
            Self::Array(items) => {
                let mut out = String::new();
                write_array(&mut out, items);
                out
            }
            Self::Other {
                text: Some(text), ..
            } => text.to_string(),
            Self::Other {
                type_name,
                text: None,
            } => format!("<{}>", type_name.to_ascii_lowercase()),
        }
    }
}

fn write_array(out: &mut String, items: &[Value]) {
    out.push('{');
    for (i, item) in items.iter().enumerate() {
        if i > 0 {
            out.push(',');
        }
        match item {
            Value::Array(inner) => write_array(out, inner),
            Value::Null | Value::Missing => out.push_str("NULL"),
            Value::Bool(_)
            | Value::Int(_)
            | Value::Float(_)
            | Value::Numeric(_)
            | Value::Uuid(_) => out.push_str(&item.display()),
            _ => push_quoted_if_needed(out, &item.display()),
        }
    }
    out.push('}');
}

/// Quote a string-like array element the way Postgres does when needed.
fn push_quoted_if_needed(out: &mut String, s: &str) {
    let needs_quote = s.is_empty()
        || s.eq_ignore_ascii_case("null")
        || s.chars()
            .any(|c| matches!(c, ',' | '{' | '}' | '"' | '\\') || c.is_whitespace());
    if !needs_quote {
        out.push_str(s);
        return;
    }
    out.push('"');
    for c in s.chars() {
        if matches!(c, '"' | '\\') {
            out.push('\\');
        }
        out.push(c);
    }
    out.push('"');
}

fn format_date(date: Date) -> String {
    format!(
        "{:04}-{:02}-{:02}",
        date.year(),
        date.month() as u8,
        date.day()
    )
}

/// `HH:MM:SS[.ffffff]`, trailing fractional zeros trimmed.
fn format_clock(time: Time) -> String {
    let mut s = format!(
        "{:02}:{:02}:{:02}",
        time.hour(),
        time.minute(),
        time.second()
    );
    if time.microsecond() != 0 {
        let _ = write!(s, ".{:06}", time.microsecond());
        while s.ends_with('0') {
            s.pop();
        }
    }
    s
}

/// Postgres style: `+00`, `+05:30`, `-08`.
fn format_offset(offset: UtcOffset) -> String {
    let (h, m, _) = offset.as_hms();
    let sign = if offset.is_negative() { '-' } else { '+' };
    if m == 0 {
        format!("{sign}{:02}", h.unsigned_abs())
    } else {
        format!("{sign}{:02}:{:02}", h.unsigned_abs(), m.unsigned_abs())
    }
}

fn format_uuid(b: &[u8; 16]) -> String {
    let mut s = String::with_capacity(36);
    for (i, byte) in b.iter().enumerate() {
        if matches!(i, 4 | 6 | 8 | 10) {
            s.push('-');
        }
        let _ = write!(s, "{byte:02x}");
    }
    s
}

#[cfg(test)]
mod tests {
    use super::*;
    use time::macros::{date, datetime, time};

    #[test]
    fn null_is_not_text() {
        assert!(Value::Null.is_null());
        assert!(Value::Missing.is_null());
        assert!(!Value::text("NULL").is_null());
        assert!(!Value::text("").is_null());
        assert_eq!(Value::Null.display(), "NULL");
        assert_eq!(Value::text("").display(), "");
    }

    #[test]
    fn displays_scalars() {
        assert_eq!(Value::Int(-42).display(), "-42");
        assert_eq!(Value::Float(1.5).display(), "1.5");
        assert_eq!(Value::Bool(true).display(), "true");
        assert_eq!(
            Value::Bytes(Arc::from(&[1u8, 2, 3][..])).display(),
            "<3 bytes>"
        );
        assert_eq!(
            Value::Uuid([
                0x55, 0x0e, 0x84, 0x00, 0xe2, 0x9b, 0x41, 0xd4, 0xa7, 0x16, 0x44, 0x66, 0x55, 0x44,
                0x00, 0x00
            ])
            .display(),
            "550e8400-e29b-41d4-a716-446655440000"
        );
        assert_eq!(Value::undecodable("INTERVAL").display(), "<interval>");
    }

    #[test]
    fn displays_temporals() {
        assert_eq!(Value::Date(date!(2020 - 01 - 15)).display(), "2020-01-15");
        assert_eq!(Value::Time(time!(09:05:00.120)).display(), "09:05:00.12");
        assert_eq!(
            Value::Timestamp(datetime!(2020-01-15 12:30)).display(),
            "2020-01-15 12:30:00"
        );
        assert_eq!(
            Value::TimestampTz(datetime!(2020-01-15 12:30 UTC)).display(),
            "2020-01-15 12:30:00+00"
        );
        assert_eq!(
            Value::TimestampTz(datetime!(2020-01-15 12:30 +5:30)).display(),
            "2020-01-15 12:30:00+05:30"
        );
    }

    #[test]
    fn displays_arrays_as_pg_literals() {
        let ints: Arc<[Value]> = [Value::Int(1), Value::Null, Value::Int(3)].into();
        assert_eq!(Value::Array(ints).display(), "{1,NULL,3}");
        let texts: Arc<[Value]> =
            [Value::text("a b"), Value::text("c"), Value::text("null")].into();
        assert_eq!(Value::Array(texts).display(), r#"{"a b",c,"null"}"#);
        let row = |a, b| Value::Array([Value::Int(a), Value::Int(b)].into());
        let grid: Arc<[Value]> = [row(1, 2), row(3, 4)].into();
        assert_eq!(Value::Array(grid).display(), "{{1,2},{3,4}}");
        assert_eq!(Value::Array(Arc::from([])).display(), "{}");
    }
}
