//! Translate the shared filter DSL into engine-specific predicates.
//!
//! Phase 1 hardening:
//!  - Operators are parsed into a typed `FilterOp` enum. Unknown
//!    operators are dropped at parse time instead of silently producing
//!    no-ops inside the builder.
//!  - Column names go through `quote_ident` (doubles embedded `"`) so
//!    a crafted column id cannot close the identifier quote and inject
//!    arbitrary SQL.
//!  - Values go through `sqlx::QueryBuilder::push_bind` rather than
//!    being string-formatted into the query text, so a crafted value
//!    cannot inject a literal either. Per-database bind encoding is
//!    picked up via the `SupportsBinds` trait bound below.
//!
//! The MongoDB path already uses `bson::doc!` for values (typed), but
//! now also rejects top-level collection-field names containing `$` so
//! users can't slip operator keys into the filter document.

use mongodb::bson::{Bson, Document, Regex, doc};
use sqlx::{Database, Encode, QueryBuilder, Type};

use super::types::FilterParam;

// ---------------------------------------------------------------------------
// Operator DSL
// ---------------------------------------------------------------------------

/// Typed view of the filter-operator strings emitted by
/// `src/components/data-table-filter`. Any operator the UI can produce
/// is represented here; everything else is discarded at parse time.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FilterOp {
    Contains,
    DoesNotContain,
    Equals,
    NotEquals,
    LessThan,
    LessOrEqual,
    GreaterThan,
    GreaterOrEqual,
    Between,
    NotBetween,
    AnyOf,
    NoneOf,
}

impl FilterOp {
    fn parse(s: &str) -> Option<Self> {
        match s {
            "contains" => Some(Self::Contains),
            "does not contain" => Some(Self::DoesNotContain),
            "is" => Some(Self::Equals),
            "is not" => Some(Self::NotEquals),
            "is less than" | "is before" => Some(Self::LessThan),
            "is less than or equal to" | "is on or before" => Some(Self::LessOrEqual),
            "is greater than" | "is after" => Some(Self::GreaterThan),
            "is greater than or equal to" | "is on or after" => Some(Self::GreaterOrEqual),
            "is between" => Some(Self::Between),
            "is not between" => Some(Self::NotBetween),
            "is any of" => Some(Self::AnyOf),
            "is none of" => Some(Self::NoneOf),
            _ => None,
        }
    }
}

/// Parse and validate a JSON-encoded filter list. Malformed payloads
/// (bad JSON, unknown operators, missing values) are dropped silently
/// so a broken filter never aborts a browse request; the user just
/// sees an unfiltered table.
pub fn parse_filters(raw: Option<&str>) -> Vec<FilterParam> {
    raw.and_then(|f| serde_json::from_str::<Vec<FilterParam>>(f).ok())
        .unwrap_or_default()
}

// ---------------------------------------------------------------------------
// SQL: identifier quoting + bound filter builder
// ---------------------------------------------------------------------------

/// Quote a SQL identifier by wrapping in `"..."` and doubling embedded
/// double-quotes. Works for both PostgreSQL and SQLite since both use
/// the ANSI `"` identifier delimiter.
///
/// Using this function (rather than inline `format!(r#""{}""#, x)`) is
/// the only identifier-injection defence we have for column, table,
/// and schema names coming from the UI.
pub fn quote_ident(name: &str) -> String {
    let escaped = name.replace('"', "\"\"");
    format!("\"{}\"", escaped)
}

/// Trait alias for the scalar types we need to be able to bind into a
/// query, for whichever database the caller is using. Both SQLite and
/// Postgres satisfy this; MongoDB doesn't go through sqlx at all.
pub trait SupportsBinds: Database
where
    for<'q> i64: Encode<'q, Self> + Type<Self>,
    for<'q> f64: Encode<'q, Self> + Type<Self>,
    for<'q> bool: Encode<'q, Self> + Type<Self>,
    for<'q> String: Encode<'q, Self> + Type<Self>,
{
}

impl SupportsBinds for sqlx::Sqlite {}
impl SupportsBinds for sqlx::Postgres {}

/// Append a `WHERE ...` clause to `qb` for the given filters. If there
/// are no effective filters, appends nothing. Values are bound through
/// `push_bind` to defeat string-literal injection.
pub fn push_sql_where<'a, DB>(qb: &mut QueryBuilder<'a, DB>, filters: &[FilterParam])
where
    DB: SupportsBinds,
    for<'q> i64: Encode<'q, DB> + Type<DB>,
    for<'q> f64: Encode<'q, DB> + Type<DB>,
    for<'q> bool: Encode<'q, DB> + Type<DB>,
    for<'q> String: Encode<'q, DB> + Type<DB>,
{
    let usable: Vec<(&FilterParam, FilterOp)> = filters
        .iter()
        .filter_map(|f| FilterOp::parse(&f.operator).map(|op| (f, op)))
        .filter(|(f, op)| {
            // Between/NotBetween need 2 args; In/NotIn need ≥1; everything
            // else needs ≥1. Reject up-front so the builder doesn't emit a
            // malformed `BETWEEN` / `IN ()`.
            match op {
                FilterOp::Between | FilterOp::NotBetween => f.values.len() >= 2,
                FilterOp::AnyOf | FilterOp::NoneOf => !f.values.is_empty(),
                _ => !f.values.is_empty(),
            }
        })
        .collect();

    if usable.is_empty() {
        return;
    }

    qb.push(" WHERE ");
    let mut first = true;
    for (f, op) in usable {
        if !first {
            qb.push(" AND ");
        }
        first = false;
        push_condition(qb, f, op);
    }
}

fn push_condition<'a, DB>(qb: &mut QueryBuilder<'a, DB>, f: &FilterParam, op: FilterOp)
where
    DB: SupportsBinds,
    for<'q> i64: Encode<'q, DB> + Type<DB>,
    for<'q> f64: Encode<'q, DB> + Type<DB>,
    for<'q> bool: Encode<'q, DB> + Type<DB>,
    for<'q> String: Encode<'q, DB> + Type<DB>,
{
    let col = quote_ident(&f.column_id);

    match op {
        FilterOp::Contains | FilterOp::DoesNotContain => {
            let raw = f
                .values
                .first()
                .and_then(|v| v.as_str())
                .unwrap_or("")
                .to_string();
            let pattern = format!("%{}%", escape_like(&raw));
            qb.push(&col);
            qb.push(if op == FilterOp::Contains {
                " LIKE "
            } else {
                " NOT LIKE "
            });
            qb.push_bind(pattern);
            qb.push(" ESCAPE '\\'");
        }
        FilterOp::Equals | FilterOp::NotEquals => {
            let v = &f.values[0];
            qb.push(&col);
            qb.push(if op == FilterOp::Equals {
                " = "
            } else {
                " != "
            });
            push_json_bind(qb, v);
        }
        FilterOp::LessThan => {
            qb.push(&col);
            qb.push(" < ");
            push_json_bind(qb, &f.values[0]);
        }
        FilterOp::LessOrEqual => {
            qb.push(&col);
            qb.push(" <= ");
            push_json_bind(qb, &f.values[0]);
        }
        FilterOp::GreaterThan => {
            qb.push(&col);
            qb.push(" > ");
            push_json_bind(qb, &f.values[0]);
        }
        FilterOp::GreaterOrEqual => {
            qb.push(&col);
            qb.push(" >= ");
            push_json_bind(qb, &f.values[0]);
        }
        FilterOp::Between | FilterOp::NotBetween => {
            qb.push(&col);
            qb.push(if op == FilterOp::Between {
                " BETWEEN "
            } else {
                " NOT BETWEEN "
            });
            push_json_bind(qb, &f.values[0]);
            qb.push(" AND ");
            push_json_bind(qb, &f.values[1]);
        }
        FilterOp::AnyOf | FilterOp::NoneOf => {
            qb.push(&col);
            qb.push(if op == FilterOp::AnyOf {
                " IN ("
            } else {
                " NOT IN ("
            });
            for (i, v) in f.values.iter().enumerate() {
                if i > 0 {
                    qb.push(", ");
                }
                push_json_bind(qb, v);
            }
            qb.push(")");
        }
    }
}

fn push_json_bind<'a, DB>(qb: &mut QueryBuilder<'a, DB>, val: &serde_json::Value)
where
    DB: SupportsBinds,
    for<'q> i64: Encode<'q, DB> + Type<DB>,
    for<'q> f64: Encode<'q, DB> + Type<DB>,
    for<'q> bool: Encode<'q, DB> + Type<DB>,
    for<'q> String: Encode<'q, DB> + Type<DB>,
{
    match val {
        serde_json::Value::Null => {
            qb.push("NULL");
        }
        serde_json::Value::Bool(b) => {
            qb.push_bind(*b);
        }
        serde_json::Value::Number(n) => {
            if let Some(i) = n.as_i64() {
                qb.push_bind(i);
            } else if let Some(f) = n.as_f64() {
                qb.push_bind(f);
            } else {
                qb.push("NULL");
            }
        }
        serde_json::Value::String(s) => {
            qb.push_bind(s.clone());
        }
        other => {
            // Array / object cells coming from filter values are not
            // something we model yet; bind the JSON text so the SQL
            // engine can reject it rather than silently matching nothing.
            qb.push_bind(other.to_string());
        }
    }
}

fn escape_like(s: &str) -> String {
    s.replace('\\', "\\\\")
        .replace('%', "\\%")
        .replace('_', "\\_")
}

/// Append `ORDER BY "col" ASC|DESC` to `qb` if both a column and a
/// valid direction are given. Direction parsing is case-insensitive
/// and anything non-`DESC` defaults to `ASC`.
pub fn push_sql_order<'a, DB>(
    qb: &mut QueryBuilder<'a, DB>,
    column: &Option<String>,
    direction: &Option<String>,
) where
    DB: Database,
{
    if let Some(col) = column {
        qb.push(" ORDER BY ");
        qb.push(quote_ident(col));
        let dir = direction
            .as_deref()
            .map(|d| d.eq_ignore_ascii_case("desc"))
            .unwrap_or(false);
        qb.push(if dir { " DESC" } else { " ASC" });
    }
}

// ---------------------------------------------------------------------------
// MongoDB
// ---------------------------------------------------------------------------

pub fn build_mongodb_filter(filters: &[FilterParam]) -> Document {
    if filters.is_empty() {
        return doc! {};
    }

    let conditions: Vec<Document> = filters
        .iter()
        .filter_map(|f| {
            let op = FilterOp::parse(&f.operator)?;
            build_mongodb_condition(f, op)
        })
        .collect();

    if conditions.is_empty() {
        return doc! {};
    }

    doc! { "$and": conditions }
}

fn build_mongodb_condition(f: &FilterParam, op: FilterOp) -> Option<Document> {
    // A collection field starting with `$` could otherwise smuggle in a
    // Mongo operator key. Reject it up-front — MongoDB doesn't support
    // `$`-prefixed top-level field names anyway, so this can only ever
    // be a malformed or adversarial filter.
    if f.column_id.starts_with('$') {
        return None;
    }
    let col = f.column_id.as_str();

    match op {
        FilterOp::Contains => {
            let val = f.values.first()?.as_str()?;
            let regex = Regex {
                pattern: regex::escape(val),
                options: "i".to_string(),
            };
            Some(doc! { col: { "$regex": regex } })
        }
        FilterOp::DoesNotContain => {
            let val = f.values.first()?.as_str()?;
            let regex = Regex {
                pattern: regex::escape(val),
                options: "i".to_string(),
            };
            Some(doc! { col: { "$not": { "$regex": regex } } })
        }
        FilterOp::Equals => {
            let val = json_to_bson(f.values.first()?);
            Some(doc! { col: { "$eq": val } })
        }
        FilterOp::NotEquals => {
            let val = json_to_bson(f.values.first()?);
            Some(doc! { col: { "$ne": val } })
        }
        FilterOp::LessThan => Some(doc! { col: { "$lt": json_to_bson(f.values.first()?) } }),
        FilterOp::LessOrEqual => Some(doc! { col: { "$lte": json_to_bson(f.values.first()?) } }),
        FilterOp::GreaterThan => Some(doc! { col: { "$gt": json_to_bson(f.values.first()?) } }),
        FilterOp::GreaterOrEqual => Some(doc! { col: { "$gte": json_to_bson(f.values.first()?) } }),
        FilterOp::Between if f.values.len() >= 2 => Some(doc! { col: {
            "$gte": json_to_bson(&f.values[0]),
            "$lte": json_to_bson(&f.values[1]),
        } }),
        FilterOp::NotBetween if f.values.len() >= 2 => Some(doc! { "$or": [
            { col: { "$lt": json_to_bson(&f.values[0]) } },
            { col: { "$gt": json_to_bson(&f.values[1]) } },
        ] }),
        FilterOp::AnyOf => {
            let vals: Vec<Bson> = f.values.iter().map(json_to_bson).collect();
            (!vals.is_empty()).then(|| doc! { col: { "$in": vals } })
        }
        FilterOp::NoneOf => {
            let vals: Vec<Bson> = f.values.iter().map(json_to_bson).collect();
            (!vals.is_empty()).then(|| doc! { col: { "$nin": vals } })
        }
        FilterOp::Between | FilterOp::NotBetween => None,
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

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn quote_ident_wraps_and_escapes() {
        assert_eq!(quote_ident("name"), r#""name""#);
        assert_eq!(quote_ident(r#"a"b"#), r#""a""b""#);
        // A crafted column that tries to inject SQL is fully contained
        // inside the quoted identifier.
        assert_eq!(
            quote_ident(r#"x"; DROP TABLE users; --"#),
            r#""x""; DROP TABLE users; --""#
        );
    }

    #[test]
    fn parse_filters_rejects_garbage() {
        assert!(parse_filters(None).is_empty());
        assert!(parse_filters(Some("not json")).is_empty());
        assert!(parse_filters(Some("{}")).is_empty());
    }

    #[test]
    fn filter_op_parse_handles_date_aliases() {
        assert_eq!(FilterOp::parse("is before"), Some(FilterOp::LessThan));
        assert_eq!(
            FilterOp::parse("is on or after"),
            Some(FilterOp::GreaterOrEqual)
        );
        assert_eq!(FilterOp::parse("random"), None);
    }

    #[test]
    fn mongodb_rejects_dollar_columns() {
        let filters = vec![FilterParam {
            column_id: "$where".to_string(),
            column_type: "text".to_string(),
            operator: "is".to_string(),
            values: vec![serde_json::json!("sleep(5000)")],
        }];
        let doc = build_mongodb_filter(&filters);
        // Rejected → empty doc
        assert_eq!(doc, doc! {});
    }

    #[test]
    fn sql_filter_uses_bind_placeholders() {
        // Using Sqlite here since it's available in the test runner
        // config. The value has a single-quote to prove it is not being
        // string-interpolated.
        let filters = vec![FilterParam {
            column_id: "name".to_string(),
            column_type: "text".to_string(),
            operator: "is".to_string(),
            values: vec![serde_json::json!("O'Brien")],
        }];

        let mut qb = QueryBuilder::<sqlx::Sqlite>::new("SELECT * FROM users");
        push_sql_where(&mut qb, &filters);
        let sql = qb.sql();
        // The bound value is not inlined; a `?` placeholder appears instead.
        assert!(sql.contains("\"name\" = ?"), "got: {sql}");
        assert!(
            !sql.contains("O'Brien"),
            "value should not appear in SQL text"
        );
    }

    #[test]
    fn sql_filter_quotes_weird_column_names() {
        let filters = vec![FilterParam {
            column_id: r#"a"b"#.to_string(),
            column_type: "text".to_string(),
            operator: "contains".to_string(),
            values: vec![serde_json::json!("x")],
        }];
        let mut qb = QueryBuilder::<sqlx::Sqlite>::new("SELECT 1");
        push_sql_where(&mut qb, &filters);
        let sql = qb.sql();
        assert!(sql.contains(r#""a""b""#), "got: {sql}");
    }
}
