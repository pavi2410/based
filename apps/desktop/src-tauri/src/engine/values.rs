//! Shared helpers for converting database values into `serde_json::Value`.
//!
//! Each engine's row types are concretely different (`SqliteRow` vs
//! `PgRow` vs BSON), so the entry points live in their respective
//! engine modules. This file houses the small bits that *are*
//! engine-agnostic so the conversion logic stays consistent:
//!
//!  - Formatting binary blobs into a `[BLOB: N bytes]` placeholder
//!    instead of a potentially huge or invalid-UTF8 string.
//!  - Wrapping `f64` into `serde_json::Number` with a single NaN/Inf
//!    policy (map to `null`).

/// Human-readable placeholder for a binary column value.
pub fn blob_marker(len: usize) -> serde_json::Value {
    serde_json::Value::String(format!("[BLOB: {} bytes]", len))
}

/// Wrap an `f64` into a `serde_json::Value`, mapping NaN / Inf to
/// `null` since JSON can't represent them.
pub fn f64_to_json(v: f64) -> serde_json::Value {
    serde_json::Number::from_f64(v)
        .map(serde_json::Value::Number)
        .unwrap_or(serde_json::Value::Null)
}
