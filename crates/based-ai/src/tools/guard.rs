//! Static read-only checks. Defense in depth: backends must also run queries in a
//! read-only transaction or connection, since text checks cannot catch every side effect.

use based_core::EngineKind;

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

/// Returns `Err(reason)` when `text` is not provably read-only for `engine`.
pub fn check(engine: EngineKind, text: &str) -> Result<(), String> {
    match engine {
        EngineKind::Postgres => check_sql(text, true),
        EngineKind::SQLite => check_sql(text, false),
        EngineKind::Redis => check_redis(text),
        EngineKind::MongoDB => check_mongo(text),
    }
}

fn check_redis(cmd: &str) -> Result<(), String> {
    let name = cmd
        .split_whitespace()
        .next()
        .unwrap_or("")
        .to_ascii_uppercase();
    if REDIS_ALLOWED.contains(&name.as_str()) {
        Ok(())
    } else {
        Err(format!(
            "redis command `{name}` is not allowed in read-only mode"
        ))
    }
}

fn check_mongo(text: &str) -> Result<(), String> {
    let value: serde_json::Value =
        serde_json::from_str(text).map_err(|e| format!("query is not valid JSON: {e}"))?;
    match forbidden_key(&value) {
        Some(op) => Err(format!(
            "mongo operator `{op}` is not allowed in read-only mode"
        )),
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

fn check_sql(sql: &str, dollar_quotes: bool) -> Result<(), String> {
    let (words, statements) = scan(sql, dollar_quotes);
    if statements > 1 {
        return Err("multiple statements are not allowed".into());
    }
    let first = words.first().map(String::as_str).unwrap_or("");
    if !SQL_START.contains(&first) {
        return Err(format!(
            "`{first}` statements are not allowed in read-only mode"
        ));
    }
    if first == "explain" && words.iter().any(|w| w == "analyze" || w == "analyse") {
        return Err("EXPLAIN ANALYZE executes the statement and is not allowed".into());
    }
    match words.iter().find(|w| SQL_FORBIDDEN.contains(&w.as_str())) {
        Some(w) => Err(format!(
            "`{w}` is not allowed in read-only mode (if it is a column or table name, quote it)"
        )),
        None => Ok(()),
    }
}

/// Lowercased bare words outside strings/identifiers/comments, plus the number of
/// non-empty statements.
fn scan(sql: &str, dollar_quotes: bool) -> (Vec<String>, usize) {
    let chars: Vec<char> = sql.chars().collect();
    let mut words = Vec::new();
    let mut statements = 0;
    let mut has_content = false;
    let mut i = 0;
    while i < chars.len() {
        let c = chars[i];
        match c {
            '\'' | '"' | '`' => {
                i = skip_quoted(&chars, i, c);
                has_content = true;
                continue;
            }
            '-' if chars.get(i + 1) == Some(&'-') => {
                while i < chars.len() && chars[i] != '\n' {
                    i += 1;
                }
                continue;
            }
            '/' if chars.get(i + 1) == Some(&'*') => {
                i += 2;
                while i < chars.len() && !(chars[i] == '*' && chars.get(i + 1) == Some(&'/')) {
                    i += 1;
                }
                i += 2;
                continue;
            }
            '$' if dollar_quotes => {
                if let Some(end) = dollar_quote_end(&chars, i) {
                    i = end;
                    has_content = true;
                    continue;
                }
            }
            ';' => {
                if has_content {
                    statements += 1;
                    has_content = false;
                }
            }
            c if c.is_ascii_alphabetic() || c == '_' => {
                let start = i;
                while i < chars.len() && (chars[i].is_ascii_alphanumeric() || chars[i] == '_') {
                    i += 1;
                }
                words.push(
                    chars[start..i]
                        .iter()
                        .collect::<String>()
                        .to_ascii_lowercase(),
                );
                has_content = true;
                continue;
            }
            c if !c.is_whitespace() => has_content = true,
            _ => {}
        }
        i += 1;
    }
    if has_content {
        statements += 1;
    }
    (words, statements)
}

/// Index just past the closing quote (doubled quotes are escapes).
fn skip_quoted(chars: &[char], start: usize, quote: char) -> usize {
    let mut i = start + 1;
    while i < chars.len() {
        if chars[i] == quote {
            if chars.get(i + 1) == Some(&quote) {
                i += 2;
                continue;
            }
            return i + 1;
        }
        i += 1;
    }
    chars.len()
}

/// For `$tag$ ... $tag$`, returns the index past the closing tag. Tags cannot start
/// with a digit, so `$1` stays a bind parameter.
fn dollar_quote_end(chars: &[char], start: usize) -> Option<usize> {
    let tag_end = chars[start + 1..].iter().position(|&c| c == '$')? + start + 1;
    let tag = &chars[start..=tag_end];
    let inner = &tag[1..tag.len() - 1];
    if inner.first().is_some_and(|c| c.is_ascii_digit())
        || !inner.iter().all(|c| c.is_ascii_alphanumeric() || *c == '_')
    {
        return None;
    }
    let body = &chars[tag_end + 1..];
    let close = body.windows(tag.len()).position(|w| w == tag);
    Some(close.map_or(chars.len(), |p| tag_end + 1 + p + tag.len()))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sql(s: &str) -> Result<(), String> {
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
    fn dollar_tags_do_not_hide_statements() {
        // SQLite has no dollar quoting: `$a$` is a bind parameter.
        let lite = |s| check(EngineKind::SQLite, s);
        assert!(lite("SELECT $a$; DELETE FROM t; SELECT $a$").is_err());
        // Postgres tags cannot start with a digit.
        assert!(sql("SELECT $1$; DELETE FROM t; SELECT $1$").is_err());
        assert!(sql("SELECT $$; DROP TABLE x$$").is_ok());
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
