//! BSON → grid cell values.

use based_core::Value;
use mongodb::bson::Bson;
use mongodb::bson::spec::BinarySubtype;
use time::OffsetDateTime;

/// Map one document field to a cell. `None` (the document has no such key) is
/// [`Value::Missing`], distinct from an explicit `null`.
pub fn bson_to_value(field: Option<&Bson>) -> Value {
    let Some(b) = field else {
        return Value::Missing;
    };
    match b {
        Bson::Null | Bson::Undefined => Value::Null,
        Bson::Boolean(v) => Value::Bool(*v),
        Bson::Int32(n) => Value::Int((*n).into()),
        Bson::Int64(n) => Value::Int(*n),
        Bson::Double(f) => Value::Float(*f),
        Bson::Decimal128(d) => Value::Numeric(d.to_string().into()),
        Bson::String(s) => Value::text(s.as_str()),
        Bson::ObjectId(oid) => Value::text(oid.to_hex()),
        Bson::DateTime(dt) => {
            let nanos = i128::from(dt.timestamp_millis()) * 1_000_000;
            match OffsetDateTime::from_unix_timestamp_nanos(nanos) {
                Ok(t) => Value::TimestampTz(t),
                Err(_) => other(b),
            }
        }
        Bson::Binary(bin) => match <[u8; 16]>::try_from(bin.bytes.as_slice()) {
            Ok(uuid) if bin.subtype == BinarySubtype::Uuid => Value::Uuid(uuid),
            _ => Value::Bytes(bin.bytes.as_slice().into()),
        },
        Bson::Document(_) | Bson::Array(_) => {
            Value::Json(b.clone().into_relaxed_extjson().to_string().into())
        }
        _ => other(b),
    }
}

/// Regex, timestamp, code, min/max key …: shown with BSON's own text.
fn other(b: &Bson) -> Value {
    Value::Other {
        type_name: format!("{:?}", b.element_type()).into(),
        text: Some(b.to_string().into()),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use mongodb::bson::{DateTime, doc, oid::ObjectId};

    #[test]
    fn missing_null_and_text_are_distinct() {
        let d = doc! { "a": Bson::Null, "b": "NULL", "c": "" };
        assert_eq!(bson_to_value(d.get("a")), Value::Null);
        assert_eq!(bson_to_value(d.get("b")), Value::text("NULL"));
        assert_eq!(bson_to_value(d.get("c")), Value::text(""));
        assert_eq!(bson_to_value(d.get("zz")), Value::Missing);
    }

    #[test]
    fn strings_are_unquoted_and_numbers_typed() {
        let oid = ObjectId::parse_str("65a1b2c3d4e5f60718293a4b").unwrap();
        let d = doc! { "s": "hi", "i": 7, "l": 8_i64, "f": 1.5, "o": oid, "t": true };
        assert_eq!(bson_to_value(d.get("s")).display(), "hi");
        assert_eq!(bson_to_value(d.get("i")), Value::Int(7));
        assert_eq!(bson_to_value(d.get("l")), Value::Int(8));
        assert_eq!(bson_to_value(d.get("f")), Value::Float(1.5));
        assert_eq!(
            bson_to_value(d.get("o")).display(),
            "65a1b2c3d4e5f60718293a4b"
        );
        assert_eq!(bson_to_value(d.get("t")), Value::Bool(true));
    }

    #[test]
    fn dates_and_nested_values() {
        let d = doc! {
            "at": DateTime::from_millis(1_579_091_400_000),
            "n": { "x": 1 },
            "arr": [1, "a"],
        };
        assert_eq!(
            bson_to_value(d.get("at")).display(),
            "2020-01-15 12:30:00+00"
        );
        assert_eq!(bson_to_value(d.get("n")), Value::Json(r#"{"x":1}"#.into()));
        assert_eq!(
            bson_to_value(d.get("arr")),
            Value::Json(r#"[1,"a"]"#.into())
        );
    }
}
