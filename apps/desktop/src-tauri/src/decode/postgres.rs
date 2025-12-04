// Copyright 2019-2023 Tauri Programme within The Commons Conservancy
// SPDX-License-Identifier: Apache-2.0
// SPDX-License-Identifier: MIT

use crate::error::Error;
use serde_json::Value as JsonValue;
use sqlx::{TypeInfo, Value, ValueRef, postgres::PgValueRef};
use time::{Date, PrimitiveDateTime, Time};

pub(crate) fn to_json(v: PgValueRef) -> Result<JsonValue, Error> {
    if v.is_null() {
        return Ok(JsonValue::Null);
    }

    let res = match v.type_info().name() {
        "TEXT" | "VARCHAR" | "CHAR" | "NAME" => match ValueRef::to_owned(&v).try_decode() {
            Ok(v) => JsonValue::String(v),
            _ => JsonValue::Null,
        },
        "FLOAT4" | "FLOAT8" | "REAL" | "DOUBLE PRECISION" => match ValueRef::to_owned(&v).try_decode::<f64>() {
            Ok(v) => JsonValue::from(v),
            _ => JsonValue::Null,
        },
        "INT2" | "INT4" | "SMALLINT" | "INT" | "INTEGER" => match ValueRef::to_owned(&v).try_decode::<i32>() {
            Ok(v) => JsonValue::Number(v.into()),
            _ => JsonValue::Null,
        },
        "INT8" | "BIGINT" => match ValueRef::to_owned(&v).try_decode::<i64>() {
            Ok(v) => JsonValue::Number(v.into()),
            _ => JsonValue::Null,
        },
        "BOOL" | "BOOLEAN" => match ValueRef::to_owned(&v).try_decode() {
            Ok(v) => JsonValue::Bool(v),
            _ => JsonValue::Null,
        },
        "DATE" => match ValueRef::to_owned(&v).try_decode::<Date>() {
            Ok(v) => JsonValue::String(v.to_string()),
            _ => JsonValue::Null,
        },
        "TIME" => match ValueRef::to_owned(&v).try_decode::<Time>() {
            Ok(v) => JsonValue::String(v.to_string()),
            _ => JsonValue::Null,
        },
        "TIMESTAMP" | "TIMESTAMPTZ" => match ValueRef::to_owned(&v).try_decode::<PrimitiveDateTime>() {
            Ok(v) => JsonValue::String(v.to_string()),
            _ => JsonValue::Null,
        },
        "BYTEA" => match ValueRef::to_owned(&v).try_decode::<Vec<u8>>() {
            Ok(v) => JsonValue::Array(v.into_iter().map(|n| JsonValue::Number(n.into())).collect()),
            _ => JsonValue::Null,
        },
        "JSON" | "JSONB" => match ValueRef::to_owned(&v).try_decode::<JsonValue>() {
            Ok(v) => v,
            _ => JsonValue::Null,
        },
        "NULL" | "VOID" => JsonValue::Null,
        _ => return Err(Error::UnsupportedDatatype(v.type_info().name().to_string())),
    };

    Ok(res)
}
