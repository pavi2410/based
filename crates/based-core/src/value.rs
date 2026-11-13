//! Typed cell values shared by every engine and the result grids.
//!
//! Engines decode into [`Value`]; the UI formats it. SQL NULL is a variant, never
//! a display string, so a text value `'NULL'` or `''` stays text.

use std::cmp::Ordering;
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

    /// Short type name for the cell inspector.
    pub fn type_label(&self) -> String {
        match self {
            Self::Null => "NULL".into(),
            Self::Missing => "MISSING".into(),
            Self::Bool(_) => "BOOLEAN".into(),
            Self::Int(_) => "INTEGER".into(),
            Self::Float(_) => "FLOAT".into(),
            Self::Numeric(_) => "NUMERIC".into(),
            Self::Text(_) => "TEXT".into(),
            Self::Json(_) => "JSON".into(),
            Self::Date(_) => "DATE".into(),
            Self::Time(_) => "TIME".into(),
            Self::Timestamp(_) => "TIMESTAMP".into(),
            Self::TimestampTz(_) => "TIMESTAMPTZ".into(),
            Self::Uuid(_) => "UUID".into(),
            Self::Bytes(_) => "BLOB".into(),
            Self::Array(_) => "ARRAY".into(),
            Self::Other { type_name, .. } => type_name.to_ascii_uppercase(),
        }
    }

    /// Numeric value for comparison, when this is a number.
    pub fn as_f64(&self) -> Option<f64> {
        match self {
            Self::Int(n) => Some(*n as f64),
            Self::Float(f) => Some(*f),
            Self::Numeric(s) => s.parse().ok(),
            _ => None,
        }
    }

    /// Grid sort order: NULL/missing last, numbers numerically, temporals
    /// chronologically, everything else by its text. Values of different kinds
    /// (a mixed Mongo field) cluster by kind.
    pub fn cmp_for_sort(&self, other: &Self) -> Ordering {
        match (self.is_null(), other.is_null()) {
            (true, true) => return Ordering::Equal,
            (true, false) => return Ordering::Greater,
            (false, true) => return Ordering::Less,
            (false, false) => {}
        }
        if let (Some(a), Some(b)) = (self.as_f64(), other.as_f64()) {
            // Exact ties (big integers past f64 precision) fall back to the text.
            return a.total_cmp(&b).then_with(|| match (self, other) {
                (Self::Int(x), Self::Int(y)) => x.cmp(y),
                _ => Ordering::Equal,
            });
        }
        match (self, other) {
            (Self::Bool(a), Self::Bool(b)) => a.cmp(b),
            (Self::Date(a), Self::Date(b)) => a.cmp(b),
            (Self::Time(a), Self::Time(b)) => a.cmp(b),
            (Self::Timestamp(a), Self::Timestamp(b)) => a.cmp(b),
            (Self::TimestampTz(a), Self::TimestampTz(b)) => a.cmp(b),
            (Self::Uuid(a), Self::Uuid(b)) => a.cmp(b),
            (Self::Bytes(a), Self::Bytes(b)) => a.cmp(b),
            (Self::Text(a), Self::Text(b)) | (Self::Json(a), Self::Json(b)) => a.cmp(b),
            _ => self
                .sort_rank()
                .cmp(&other.sort_rank())
                .then_with(|| self.display().cmp(&other.display())),
        }
    }

    fn sort_rank(&self) -> u8 {
        match self {
            Self::Bool(_) => 0,
            Self::Int(_) | Self::Float(_) | Self::Numeric(_) => 1,
            Self::Date(_) | Self::Time(_) | Self::Timestamp(_) | Self::TimestampTz(_) => 2,
            Self::Text(_) | Self::Uuid(_) => 3,
            Self::Json(_) | Self::Array(_) => 4,
            Self::Bytes(_) => 5,
            Self::Other { .. } => 6,
            Self::Null | Self::Missing => 7,
        }
    }

    /// JSON for export and AI tools. Numbers and booleans stay typed; a decimal
    /// with more than 15 significant digits stays a string; bytes are `\x…` hex.
    pub fn to_json(&self) -> serde_json::Value {
        use serde_json::Value as J;
        match self {
            Self::Null | Self::Missing => J::Null,
            Self::Bool(b) => J::Bool(*b),
            Self::Int(n) => J::from(*n),
            Self::Float(f) => serde_json::Number::from_f64(*f)
                .map(J::Number)
                .unwrap_or_else(|| J::String(f.to_string())),
            Self::Numeric(s) => numeric_json(s).unwrap_or_else(|| J::String(s.to_string())),
            Self::Json(s) => serde_json::from_str(s).unwrap_or_else(|_| J::String(s.to_string())),
            Self::Bytes(b) => {
                let mut hex = String::with_capacity(2 + b.len() * 2);
                hex.push_str("\\x");
                for byte in b.iter() {
                    let _ = write!(hex, "{byte:02x}");
                }
                J::String(hex)
            }
            Self::Array(items) => J::Array(items.iter().map(Value::to_json).collect()),
            Self::Text(s) => J::String(s.to_string()),
            _ => J::String(self.display()),
        }
    }

    /// Whether a decimal survives a trip through `f64` (≤ 15 significant digits),
    /// so spreadsheets can take it as a number.
    pub fn numeric_fits_f64(s: &str) -> bool {
        let digits = s
            .trim_start_matches(['-', '+'])
            .chars()
            .filter(char::is_ascii_digit)
            .skip_while(|c| *c == '0')
            .count();
        digits <= 15 && s.parse::<f64>().is_ok_and(f64::is_finite)
    }
}

fn numeric_json(s: &str) -> Option<serde_json::Value> {
    if let Ok(n) = s.parse::<i64>() {
        return Some(n.into());
    }
    // Same rule for every row of a column, so a `numeric(10,2)` column is all
    // numbers (`1.5` for `1.50`), not numbers mixed with strings.
    if !Value::numeric_fits_f64(s) {
        return None;
    }
    serde_json::Number::from_f64(s.parse().ok()?).map(serde_json::Value::Number)
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

    fn sorted(mut v: Vec<Value>) -> Vec<String> {
        v.sort_by(Value::cmp_for_sort);
        v.iter().map(Value::display).collect()
    }

    #[test]
    fn sorts_numbers_numerically_and_nulls_last() {
        let got = sorted(vec![
            Value::Int(10),
            Value::Null,
            Value::Float(2.5),
            Value::Numeric("3".into()),
            Value::Missing,
            Value::Int(-1),
        ]);
        assert_eq!(got, ["-1", "2.5", "3", "10", "NULL", "—"]);
    }

    #[test]
    fn text_null_sorts_as_text() {
        let got = sorted(vec![Value::Null, Value::text("NULL"), Value::text("A")]);
        assert_eq!(got, ["A", "NULL", "NULL"]);
        let mut v = vec![Value::Null, Value::text("NULL")];
        v.sort_by(Value::cmp_for_sort);
        assert_eq!(v, [Value::text("NULL"), Value::Null]);
    }

    #[test]
    fn sorts_temporals_and_bools() {
        assert_eq!(
            sorted(vec![
                Value::Date(date!(2021 - 01 - 01)),
                Value::Date(date!(2020 - 12 - 31))
            ]),
            ["2020-12-31", "2021-01-01"]
        );
        assert_eq!(
            sorted(vec![Value::Bool(true), Value::Bool(false)]),
            ["false", "true"]
        );
    }

    #[test]
    fn mixed_kinds_cluster() {
        let got = sorted(vec![
            Value::text("b"),
            Value::Int(2),
            Value::text("a"),
            Value::Int(1),
        ]);
        assert_eq!(got, ["1", "2", "a", "b"]);
    }

    #[test]
    fn json_keeps_types() {
        use serde_json::json;
        assert_eq!(Value::Null.to_json(), json!(null));
        assert_eq!(Value::text("NULL").to_json(), json!("NULL"));
        assert_eq!(Value::Int(42).to_json(), json!(42));
        assert_eq!(Value::Float(1.5).to_json(), json!(1.5));
        assert_eq!(Value::Bool(true).to_json(), json!(true));
        assert_eq!(Value::Numeric("123.45".into()).to_json(), json!(123.45));
        assert_eq!(Value::Numeric("1.50".into()).to_json(), json!(1.5));
        assert_eq!(
            Value::Numeric("12345678901234567890.1".into()).to_json(),
            json!("12345678901234567890.1")
        );
        assert_eq!(Value::Json(r#"{"a":1}"#.into()).to_json(), json!({"a": 1}));
        assert_eq!(
            Value::Bytes(Arc::from(&[0xde, 0xad][..])).to_json(),
            json!("\\xdead")
        );
        assert_eq!(
            Value::Array([Value::Int(1), Value::Null].into()).to_json(),
            json!([1, null])
        );
        assert_eq!(
            Value::Date(date!(2020 - 01 - 15)).to_json(),
            json!("2020-01-15")
        );
    }
}
