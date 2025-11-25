// Copyright 2019-2023 Tauri Programme within The Commons Conservancy
// SPDX-License-Identifier: Apache-2.0
// SPDX-License-Identifier: MIT

use crate::error::Error;
use serde_json::Value as JsonValue;
use sqlx::{TypeInfo, Value, ValueRef, sqlite::SqliteValueRef};
use time::{Date, PrimitiveDateTime, Time};

pub(crate) fn to_json(v: SqliteValueRef) -> Result<JsonValue, Error> {
    if v.is_null() {
        return Ok(JsonValue::Null);
    }

    let res = match v.type_info().name() {
        "TEXT" => match v.to_owned().try_decode() {
            Ok(v) => JsonValue::String(v),
            _ => JsonValue::Null,
        },
        "REAL" => match v.to_owned().try_decode::<f64>() {
            Ok(v) => JsonValue::from(v),
            _ => JsonValue::Null,
        },
        "INTEGER" | "NUMERIC" => match v.to_owned().try_decode::<i64>() {
            Ok(v) => JsonValue::Number(v.into()),
            _ => JsonValue::Null,
        },
        "BOOLEAN" => match v.to_owned().try_decode() {
            Ok(v) => JsonValue::Bool(v),
            _ => JsonValue::Null,
        },
        "DATE" => match v.to_owned().try_decode::<Date>() {
            Ok(v) => JsonValue::String(v.to_string()),
            _ => JsonValue::Null,
        },
        "TIME" => match v.to_owned().try_decode::<Time>() {
            Ok(v) => JsonValue::String(v.to_string()),
            _ => JsonValue::Null,
        },
        "DATETIME" => match v.to_owned().try_decode::<PrimitiveDateTime>() {
            Ok(v) => JsonValue::String(v.to_string()),
            _ => JsonValue::Null,
        },
        "BLOB" => match v.to_owned().try_decode::<Vec<u8>>() {
            Ok(v) => JsonValue::Array(v.into_iter().map(|n| JsonValue::Number(n.into())).collect()),
            _ => JsonValue::Null,
        },
        "NULL" => JsonValue::Null,
        _ => return Err(Error::UnsupportedDatatype(v.type_info().name().to_string())),
    };

    Ok(res)
}
