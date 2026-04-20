//! Translate the shared `FilterParam` DSL into engine-specific
//! predicates.
//!
//! This module is the single place the app converts user-authored
//! filter rows into a `WHERE` clause / Mongo filter document. Phase 1
//! todo 2 hardens this into a proper AST with bound parameters; for now
//! it keeps the original string-building logic from
//! `project_db_commands.rs` but behind a single import point so every
//! engine uses the same mapping of operators to SQL / BSON.

use mongodb::bson::{Bson, Document, Regex, doc};

use super::types::FilterParam;

// ---------------------------------------------------------------------------
// SQL
// ---------------------------------------------------------------------------

/// Build a SQL `WHERE` clause from the filter DSL. Returns an empty
/// string when there are no filters so callers can concatenate it
/// unconditionally.
pub fn build_sql_where_clause(filters: &[FilterParam]) -> String {
    if filters.is_empty() {
        return String::new();
    }

    let conditions: Vec<String> = filters.iter().filter_map(build_sql_condition).collect();

    if conditions.is_empty() {
        return String::new();
    }

    format!(" WHERE {}", conditions.join(" AND "))
}

fn build_sql_condition(filter: &FilterParam) -> Option<String> {
    let col = format!(r#""{}""#, filter.column_id);

    match filter.operator.as_str() {
        "contains" => {
            let val = filter.values.first()?.as_str()?;
            Some(format!("{} LIKE '%{}%'", col, escape_sql_like(val)))
        }
        "does not contain" => {
            let val = filter.values.first()?.as_str()?;
            Some(format!("{} NOT LIKE '%{}%'", col, escape_sql_like(val)))
        }
        "is" => {
            let val = &filter.values.first()?;
            Some(format!("{} = {}", col, sql_value(val)))
        }
        "is not" => {
            let val = &filter.values.first()?;
            Some(format!("{} != {}", col, sql_value(val)))
        }
        "is less than" | "is before" => {
            let val = &filter.values.first()?;
            Some(format!("{} < {}", col, sql_value(val)))
        }
        "is less than or equal to" | "is on or before" => {
            let val = &filter.values.first()?;
            Some(format!("{} <= {}", col, sql_value(val)))
        }
        "is greater than" | "is after" => {
            let val = &filter.values.first()?;
            Some(format!("{} > {}", col, sql_value(val)))
        }
        "is greater than or equal to" | "is on or after" => {
            let val = &filter.values.first()?;
            Some(format!("{} >= {}", col, sql_value(val)))
        }
        "is between" if filter.values.len() >= 2 => {
            let v1 = sql_value(&filter.values[0]);
            let v2 = sql_value(&filter.values[1]);
            Some(format!("{} BETWEEN {} AND {}", col, v1, v2))
        }
        "is not between" if filter.values.len() >= 2 => {
            let v1 = sql_value(&filter.values[0]);
            let v2 = sql_value(&filter.values[1]);
            Some(format!("{} NOT BETWEEN {} AND {}", col, v1, v2))
        }
        "is any of" => {
            let vals: Vec<String> = filter.values.iter().map(sql_value).collect();
            (!vals.is_empty()).then(|| format!("{} IN ({})", col, vals.join(", ")))
        }
        "is none of" => {
            let vals: Vec<String> = filter.values.iter().map(sql_value).collect();
            (!vals.is_empty()).then(|| format!("{} NOT IN ({})", col, vals.join(", ")))
        }
        _ => None,
    }
}

fn escape_sql_like(s: &str) -> String {
    s.replace('\\', "\\\\")
        .replace('%', "\\%")
        .replace('_', "\\_")
        .replace('\'', "''")
}

fn sql_value(val: &serde_json::Value) -> String {
    match val {
        serde_json::Value::Null => "NULL".to_string(),
        serde_json::Value::Bool(b) => if *b { "1" } else { "0" }.to_string(),
        serde_json::Value::Number(n) => n.to_string(),
        serde_json::Value::String(s) => format!("'{}'", s.replace('\'', "''")),
        _ => "NULL".to_string(),
    }
}

/// Parse the JSON-encoded filter DSL from `BrowseOptions.filters`.
/// Returns an empty vec on parse errors so a malformed filter never
/// aborts the whole browse request.
pub fn parse_filters(raw: Option<&str>) -> Vec<FilterParam> {
    raw.and_then(|f| serde_json::from_str(f).ok())
        .unwrap_or_default()
}

// ---------------------------------------------------------------------------
// MongoDB
// ---------------------------------------------------------------------------

pub fn build_mongodb_filter(filters: &[FilterParam]) -> Document {
    if filters.is_empty() {
        return doc! {};
    }

    let conditions: Vec<Document> = filters.iter().filter_map(build_mongodb_condition).collect();

    if conditions.is_empty() {
        return doc! {};
    }

    doc! { "$and": conditions }
}

fn build_mongodb_condition(filter: &FilterParam) -> Option<Document> {
    let col = &filter.column_id;

    match filter.operator.as_str() {
        "contains" => {
            let val = filter.values.first()?.as_str()?;
            let regex = Regex {
                pattern: regex::escape(val),
                options: "i".to_string(),
            };
            Some(doc! { col: { "$regex": regex } })
        }
        "does not contain" => {
            let val = filter.values.first()?.as_str()?;
            let regex = Regex {
                pattern: regex::escape(val),
                options: "i".to_string(),
            };
            Some(doc! { col: { "$not": { "$regex": regex } } })
        }
        "is" => {
            let val = json_to_bson(filter.values.first()?);
            Some(doc! { col: { "$eq": val } })
        }
        "is not" => {
            let val = json_to_bson(filter.values.first()?);
            Some(doc! { col: { "$ne": val } })
        }
        "is less than" | "is before" => {
            let val = json_to_bson(filter.values.first()?);
            Some(doc! { col: { "$lt": val } })
        }
        "is less than or equal to" | "is on or before" => {
            let val = json_to_bson(filter.values.first()?);
            Some(doc! { col: { "$lte": val } })
        }
        "is greater than" | "is after" => {
            let val = json_to_bson(filter.values.first()?);
            Some(doc! { col: { "$gt": val } })
        }
        "is greater than or equal to" | "is on or after" => {
            let val = json_to_bson(filter.values.first()?);
            Some(doc! { col: { "$gte": val } })
        }
        "is between" if filter.values.len() >= 2 => {
            let v1 = json_to_bson(&filter.values[0]);
            let v2 = json_to_bson(&filter.values[1]);
            Some(doc! { col: { "$gte": v1, "$lte": v2 } })
        }
        "is not between" if filter.values.len() >= 2 => {
            let v1 = json_to_bson(&filter.values[0]);
            let v2 = json_to_bson(&filter.values[1]);
            Some(doc! { "$or": [{ col: { "$lt": v1 } }, { col: { "$gt": v2 } }] })
        }
        "is any of" => {
            let vals: Vec<Bson> = filter.values.iter().map(json_to_bson).collect();
            (!vals.is_empty()).then(|| doc! { col: { "$in": vals } })
        }
        "is none of" => {
            let vals: Vec<Bson> = filter.values.iter().map(json_to_bson).collect();
            (!vals.is_empty()).then(|| doc! { col: { "$nin": vals } })
        }
        _ => None,
    }
}

fn json_to_bson(val: &serde_json::Value) -> Bson {
    match val {
        serde_json::Value::Null => Bson::Null,
        serde_json::Value::Bool(b) => Bson::Boolean(*b),
        serde_json::Value::Number(n) => {
            if let Some(i) = n.as_i64() {
                Bson::Int64(i)
            } else if let Some(f) = n.as_f64() {
                Bson::Double(f)
            } else {
                Bson::Null
            }
        }
        serde_json::Value::String(s) => Bson::String(s.clone()),
        serde_json::Value::Array(arr) => Bson::Array(arr.iter().map(json_to_bson).collect()),
        serde_json::Value::Object(obj) => {
            let document: Document = obj
                .iter()
                .map(|(k, v)| (k.clone(), json_to_bson(v)))
                .collect();
            Bson::Document(document)
        }
    }
}

/// Derive an ORDER BY clause for SQL.
pub fn order_clause_sql(column: &Option<String>, direction: &Option<String>) -> String {
    match (column, direction) {
        (Some(col), Some(dir)) => {
            let dir = if dir.eq_ignore_ascii_case("desc") {
                "DESC"
            } else {
                "ASC"
            };
            format!(r#" ORDER BY "{}" {}"#, col, dir)
        }
        _ => String::new(),
    }
}
