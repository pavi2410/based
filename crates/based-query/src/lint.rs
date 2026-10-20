//! Syntax checking for SQL scripts (no UI, no database).

use based_core::EngineKind;
use sqlparser::dialect::{Dialect, GenericDialect, PostgreSqlDialect, SQLiteDialect};
use sqlparser::parser::Parser;

use crate::statements_in_script;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Severity {
    Error,
    Warning,
}

/// A problem at a byte range of the script.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Lint {
    pub start: usize,
    pub end: usize,
    pub severity: Severity,
    pub message: String,
}

fn dialect(engine: EngineKind) -> Box<dyn Dialect> {
    match engine {
        EngineKind::Postgres => Box::new(PostgreSqlDialect {}),
        EngineKind::SQLite => Box::new(SQLiteDialect {}),
        _ => Box::new(GenericDialect {}),
    }
}

/// Parse each statement of `script`; report syntax errors at the offending token.
pub fn syntax_lints(script: &str, engine: EngineKind) -> Vec<Lint> {
    let dialect = dialect(engine);
    let mut out = Vec::new();
    for stmt in statements_in_script(script) {
        let text = stmt.text(script);
        let Err(err) = Parser::parse_sql(dialect.as_ref(), text) else {
            continue;
        };
        let msg = err.to_string();
        let raw = &script[stmt.start..stmt.end];
        let base = stmt.start + raw.len() - raw.trim_start().len();
        let (start, end) = match locate(&msg, text) {
            Some((s, e)) => (base + s, base + e),
            None => (base, base + text.len()),
        };
        out.push(Lint {
            start,
            end,
            severity: Severity::Error,
            message: clean(&msg),
        });
    }
    out
}

/// Byte range of the token at `Line: L, Column: C` (1-based, chars) inside `text`.
fn locate(msg: &str, text: &str) -> Option<(usize, usize)> {
    let tail = &msg[msg.rfind("Line: ")? + 6..];
    let (line, col) = tail.split_once(", Column: ")?;
    let line: usize = line.trim().parse().ok()?;
    let col: usize = col.trim().parse().ok()?;
    let line_start: usize = text
        .split_inclusive('\n')
        .take(line.checked_sub(1)?)
        .map(str::len)
        .sum();
    let rest = &text[line_start..];
    let start = line_start
        + rest
            .char_indices()
            .nth(col.checked_sub(1)?)
            .map_or(rest.len(), |(i, _)| i);
    let word = text[start..]
        .char_indices()
        .find(|&(i, c)| i > 0 && !(c.is_alphanumeric() || c == '_'))
        .map_or(text.len(), |(i, _)| start + i);
    Some((start, word.max(start + 1).min(text.len())))
}

/// Drop the trailing ` at Line: .., Column: ..` the editor already shows visually.
fn clean(msg: &str) -> String {
    let msg = msg.strip_prefix("sql parser error: ").unwrap_or(msg);
    msg.rfind(" at Line: ")
        .map_or(msg, |i| &msg[..i])
        .to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn valid_sql_has_no_lints() {
        assert!(syntax_lints("SELECT a FROM t; SELECT 1", EngineKind::Postgres).is_empty());
    }

    #[test]
    fn reports_error_at_token() {
        let sql = "SELECT 1;\nSELECT a FROM t WHERE";
        let lints = syntax_lints(sql, EngineKind::Postgres);
        assert_eq!(lints.len(), 1);
        assert!(lints[0].start >= 10, "{lints:?}");
        assert!(lints[0].end <= sql.len());
    }
}
