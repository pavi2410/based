//! Static read-only checks. Defense in depth: backends must also run queries in a
//! read-only transaction or connection, since text checks cannot catch every side effect.

use std::fmt;

use based_core::EngineKind;
use based_query::{has_ambiguous_quoting, keywords, split_statements};

const SQL_START: &[&str] = &["select", "with", "values", "show", "explain"];
const SQL_FORBIDDEN: &[&str] = &[
    "insert", "update", "delete", "merge", "into", "truncate", "drop", "alter", "create", "grant",
    "revoke", "copy", "call",
];
const REDIS_ALLOWED: &[&str] = &[
    "GET",
    "MGET",
    "STRLEN",
    "EXISTS",
    "TYPE",
    "TTL",
    "PTTL",
    "HGET",
    "HGETALL",
    "HMGET",
    "HKEYS",
    "HVALS",
    "HLEN",
    "LRANGE",
    "LLEN",
    "LINDEX",
    "SMEMBERS",
    "SCARD",
    "SISMEMBER",
    "ZRANGE",
    "ZCARD",
    "ZSCORE",
    "SCAN",
    "SSCAN",
    "HSCAN",
    "ZSCAN",
    "DBSIZE",
];
const MONGO_FORBIDDEN: &[&str] = &["$out", "$merge", "$function", "$accumulator", "$where"];

/// Why a query was refused; `Display` is the text shown to the model.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Refusal {
    NotJson(String),
    RedisCommand(String),
    MongoOperator(&'static str),
    MultipleStatements,
    /// Quoting the checker can't read the way the database will (`$1$ … $1$`).
    AmbiguousQuoting,
    Statement(String),
    ExplainAnalyze,
    Keyword(String),
}

impl fmt::Display for Refusal {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::NotJson(e) => write!(f, "query is not valid JSON: {e}"),
            Self::RedisCommand(name) => {
                write!(f, "redis command `{name}` is not allowed in read-only mode")
            }
            Self::MongoOperator(op) => {
                write!(f, "mongo operator `{op}` is not allowed in read-only mode")
            }
            Self::MultipleStatements => f.write_str("multiple statements are not allowed"),
            Self::AmbiguousQuoting => {
                f.write_str("dollar-quote tags cannot start with a digit; use $$ or $tag$")
            }
            Self::Statement(first) => {
                write!(f, "`{first}` statements are not allowed in read-only mode")
            }
            Self::ExplainAnalyze => {
                f.write_str("EXPLAIN ANALYZE executes the statement and is not allowed")
            }
            Self::Keyword(w) => write!(
                f,
                "`{w}` is not allowed in read-only mode (if it is a column or table name, quote it)"
            ),
        }
    }
}

impl From<Refusal> for String {
    fn from(r: Refusal) -> Self {
        r.to_string()
    }
}

/// Returns `Err(refusal)` when `text` is not provably read-only for `engine`.
pub fn check(engine: EngineKind, text: &str) -> Result<(), Refusal> {
    match engine {
        EngineKind::Postgres | EngineKind::SQLite => check_sql(text, engine),
        EngineKind::Redis => check_redis(text),
        EngineKind::MongoDB => check_mongo(text),
    }
}

fn check_redis(cmd: &str) -> Result<(), Refusal> {
    let name = cmd
        .split_whitespace()
        .next()
        .unwrap_or("")
        .to_ascii_uppercase();
    if REDIS_ALLOWED.contains(&name.as_str()) {
        Ok(())
    } else {
        Err(Refusal::RedisCommand(name))
    }
}

fn check_mongo(text: &str) -> Result<(), Refusal> {
    let value: serde_json::Value =
        serde_json::from_str(text).map_err(|e| Refusal::NotJson(e.to_string()))?;
    match forbidden_key(&value) {
        Some(op) => Err(Refusal::MongoOperator(op)),
        None => Ok(()),
    }
}

/// Walks decoded JSON keys, so escapes like `\u0024out` cannot hide an operator.
fn forbidden_key(v: &serde_json::Value) -> Option<&'static str> {
    use serde_json::Value;
    match v {
        Value::Object(map) => map.iter().find_map(|(k, v)| {
            MONGO_FORBIDDEN
                .iter()
                .find(|op| k.eq_ignore_ascii_case(op))
                .copied()
                .or_else(|| forbidden_key(v))
        }),
        Value::Array(items) => items.iter().find_map(forbidden_key),
        _ => None,
    }
}

fn check_sql(sql: &str, engine: EngineKind) -> Result<(), Refusal> {
    let postgres = engine == EngineKind::Postgres;
    // The same tokenizer the editor uses to split and run statements.
    if has_ambiguous_quoting(sql, engine) {
        return Err(Refusal::AmbiguousQuoting);
    }
    let words = keywords(sql, engine);
    if split_statements(sql, engine).len() > 1 {
        return Err(Refusal::MultipleStatements);
    }
    let first = words.first().map(String::as_str).unwrap_or("");
    if !SQL_START.contains(&first) || (first == "show" && !postgres) {
        return Err(Refusal::Statement(first.to_string()));
    }
    if first == "explain" && words.iter().any(|w| w == "analyze" || w == "analyse") {
        return Err(Refusal::ExplainAnalyze);
    }
    match words.iter().find(|w| SQL_FORBIDDEN.contains(&w.as_str())) {
        Some(w) => Err(Refusal::Keyword(w.clone())),
        None => Ok(()),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sql(s: &str) -> Result<(), Refusal> {
        check(EngineKind::Postgres, s)
    }

    #[test]
    fn allows_reads() {
        for q in [
            "SELECT * FROM users;",
            "  select 1",
            "WITH t AS (SELECT 1) SELECT * FROM t",
            "EXPLAIN SELECT * FROM orders",
            "SELECT updated_at, 'drop table x' FROM t -- delete everything",
            "SELECT \"insert\" FROM t /* update */",
            "SELECT $$; DROP TABLE x$$",
            "SELECT replace(name, 'a', 'b') FROM t",
        ] {
            assert_eq!(sql(q), Ok(()), "{q}");
        }
    }

    #[test]
    fn blocks_writes() {
        for q in [
            "DELETE FROM users",
            "insert into t values (1)",
            "UPDATE t SET a = 1",
            "DROP TABLE t",
            "SELECT 1; DROP TABLE t",
            "SELECT * INTO copy FROM t",
            "SELECT * FROM t FOR UPDATE",
            "WITH d AS (DELETE FROM t RETURNING *) SELECT * FROM d",
            "EXPLAIN ANALYZE SELECT 1",
            "EXPLAIN (ANALYZE) DELETE FROM t",
            "",
            "PRAGMA writable_schema = 1",
        ] {
            assert!(sql(q).is_err(), "{q}");
        }
    }

    #[test]
    fn refusals_are_typed() {
        assert_eq!(sql("a; b"), Err(Refusal::MultipleStatements));
        assert_eq!(sql("DROP TABLE t"), Err(Refusal::Statement("drop".into())));
        assert_eq!(
            check(EngineKind::Redis, "FLUSHALL"),
            Err(Refusal::RedisCommand("FLUSHALL".into()))
        );
        assert_eq!(
            check(EngineKind::MongoDB, r#"[{"$out":"x"}]"#),
            Err(Refusal::MongoOperator("$out"))
        );
    }

    #[test]
    fn show_is_postgres_only() {
        assert!(sql("SHOW search_path").is_ok());
        assert!(check(EngineKind::SQLite, "SHOW tables").is_err());
    }

    #[test]
    fn dollar_tags_do_not_hide_statements() {
        // SQLite has no dollar quoting: `$a$` is a bind parameter.
        let lite = |s| check(EngineKind::SQLite, s);
        assert!(lite("SELECT $a$; DELETE FROM t; SELECT $a$").is_err());
        // Postgres tags cannot start with a digit.
        assert!(sql("SELECT $1$; DELETE FROM t; SELECT $1$").is_err());
        assert!(sql("SELECT $$; DROP TABLE x$$").is_ok());
    }

    #[test]
    fn lexer_quirks_do_not_hide_statements() {
        // A backslash in a plain string is literal in both databases.
        for engine in [EngineKind::Postgres, EngineKind::SQLite] {
            assert!(
                check(engine, r"SELECT '\'; DELETE FROM t; SELECT '\'").is_err(),
                "{engine:?}"
            );
        }
        // SQLite has no nested comments: `*/` closes the first `/*`.
        assert!(
            check(
                EngineKind::SQLite,
                "SELECT 1 /* /* */; DELETE FROM t; -- */"
            )
            .is_err()
        );
    }

    #[test]
    fn quotes_do_not_hide_second_statement() {
        assert!(sql("SELECT 'a'; DELETE FROM t").is_err());
        assert!(sql("SELECT 'it''s'; SELECT 2").is_err());
    }

    #[test]
    fn redis_allowlist() {
        assert!(check(EngineKind::Redis, "get user:1").is_ok());
        assert!(check(EngineKind::Redis, "SCAN 0 MATCH a*").is_ok());
        assert!(check(EngineKind::Redis, "FLUSHALL").is_err());
        assert!(check(EngineKind::Redis, "KEYS *").is_err());
    }

    #[test]
    fn mongo_blocks_write_stages() {
        assert!(check(EngineKind::MongoDB, r#"[{"$match":{"a":1}}]"#).is_ok());
        assert!(check(EngineKind::MongoDB, r#"[{"$out":"x"}]"#).is_err());
        assert!(check(EngineKind::MongoDB, r#"[{"\u0024out":"x"}]"#).is_err());
        assert!(
            check(
                EngineKind::MongoDB,
                r#"{"collection":"c","pipeline":[{"$lookup":{"pipeline":[{"$merge":"x"}]}}]}"#
            )
            .is_err()
        );
        assert!(check(EngineKind::MongoDB, "not json").is_err());
    }
}
