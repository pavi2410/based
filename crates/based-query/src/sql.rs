//! SQL text structure from `sqlparser`'s tokenizer: statement boundaries and
//! the bare words a script uses. One lexer for running, linting, completion and
//! the write checks, so a `;` inside a string, comment or `$$` body is never a split.

use std::ops::Range;

use based_core::EngineKind;
use sqlparser::dialect::{Dialect, GenericDialect, PostgreSqlDialect, SQLiteDialect};
use sqlparser::tokenizer::{Token, Tokenizer, Whitespace};

pub(crate) fn dialect(engine: EngineKind) -> Box<dyn Dialect> {
    match engine {
        EngineKind::Postgres => Box::new(PostgreSqlDialect {}),
        EngineKind::SQLite => Box::new(SQLiteDialect {}),
        _ => Box::new(GenericDialect {}),
    }
}

/// Byte offset of a 1-based (line, char column) position in `text`.
pub(crate) fn byte_offset(text: &str, line: u64, column: u64) -> usize {
    let line_start: usize = text
        .split_inclusive('\n')
        .take(usize::try_from(line.saturating_sub(1)).unwrap_or(usize::MAX))
        .map(str::len)
        .sum();
    let rest = &text[line_start.min(text.len())..];
    let col = usize::try_from(column.saturating_sub(1)).unwrap_or(usize::MAX);
    line_start + rest.char_indices().nth(col).map_or(rest.len(), |(i, _)| i)
}

/// Tokens with their byte ranges, and where lexing stopped when the text has
/// an unterminated string or comment (everything from there is one opaque run).
struct Lexed {
    tokens: Vec<(Token, Range<usize>)>,
    tail: Option<usize>,
}

fn lex(script: &str, engine: EngineKind) -> Lexed {
    let dialect = dialect(engine);
    let with_ranges = |text: &str| {
        Tokenizer::new(dialect.as_ref(), text)
            .tokenize_with_location()
            .map(|toks| {
                toks.into_iter()
                    .map(|t| {
                        let start = byte_offset(text, t.span.start.line, t.span.start.column);
                        let end = byte_offset(text, t.span.end.line, t.span.end.column);
                        (t.token, start..end.max(start))
                    })
                    .collect::<Vec<_>>()
            })
    };
    match with_ranges(script) {
        Ok(tokens) => Lexed { tokens, tail: None },
        Err(e) => {
            let at = byte_offset(script, e.location.line, e.location.column).min(script.len());
            let tokens = with_ranges(&script[..at]).unwrap_or_default();
            let tail = if tokens.is_empty() && at > 0 { 0 } else { at };
            Lexed {
                tokens,
                tail: Some(tail),
            }
        }
    }
}

fn is_comment(t: &Token) -> bool {
    matches!(
        t,
        Token::Whitespace(Whitespace::SingleLineComment { .. } | Whitespace::MultiLineComment(_))
    )
}

/// Byte ranges of each statement that has code in it, without the closing `;`.
/// A range starts at its first non-space token (a leading comment belongs to the
/// statement after it); comment-only stretches are not statements.
pub fn split_statements(script: &str, engine: EngineKind) -> Vec<Range<usize>> {
    let lexed = lex(script, engine);
    let mut out = Vec::new();
    let mut seg: Option<Range<usize>> = None;
    let mut has_code = false;
    let mut flush = |seg: &mut Option<Range<usize>>, has_code: &mut bool| {
        if let Some(r) = seg.take().filter(|_| *has_code) {
            out.push(r);
        }
        *has_code = false;
    };
    for (token, range) in lexed.tokens {
        match token {
            Token::SemiColon => flush(&mut seg, &mut has_code),
            Token::Whitespace(_) if !is_comment(&token) => {}
            other => {
                has_code |= !is_comment(&other);
                seg = Some(seg.map_or(range.clone(), |s| s.start..range.end));
            }
        }
    }
    if let Some(tail) = lexed.tail {
        let end = tail + script[tail..].trim_end().len();
        seg = Some(seg.map_or(tail..end, |s| s.start..end));
        has_code = true;
    }
    flush(&mut seg, &mut has_code);
    out
}

/// The statement containing `offset`, or the closest one before it (cursor in a
/// gap or after a trailing `;`). The first statement when the cursor is before all.
pub fn statement_at(script: &str, engine: EngineKind, offset: usize) -> Option<Range<usize>> {
    let stmts = split_statements(script, engine);
    stmts
        .iter()
        .find(|r| r.start <= offset && offset <= r.end + 1)
        .or_else(|| stmts.iter().rev().find(|r| r.end < offset))
        .or_else(|| stmts.first())
        .cloned()
}

/// Lower-cased bare words in order: keywords and unquoted identifiers, never the
/// inside of strings, quoted identifiers or comments.
pub fn keywords(script: &str, engine: EngineKind) -> Vec<String> {
    lex(script, engine)
        .tokens
        .into_iter()
        .filter_map(|(t, _)| match t {
            Token::Word(w) if w.quote_style.is_none() => Some(w.value.to_ascii_lowercase()),
            _ => None,
        })
        .collect()
}

/// Whether the tokenizer read a `$tag$` quote that Postgres would not: a tag
/// starting with a digit (`$1$`) is a bind parameter there, so the two disagree
/// about where statements end. Callers that must be conservative refuse these.
pub fn has_ambiguous_quoting(script: &str, engine: EngineKind) -> bool {
    engine == EngineKind::Postgres
        && lex(script, engine).tokens.iter().any(|(t, _)| {
            matches!(t, Token::DollarQuotedString(d)
                if d.tag.as_deref().is_some_and(|tag| tag.starts_with(|c: char| c.is_ascii_digit())))
        })
}

/// [`keywords`] minus function names (a word followed by `(`), so
/// `select replace(name, 'a', 'b')` doesn't read as a REPLACE statement.
pub(crate) fn command_words(script: &str, engine: EngineKind) -> Vec<String> {
    let tokens: Vec<Token> = lex(script, engine)
        .tokens
        .into_iter()
        .map(|(t, _)| t)
        .filter(|t| !matches!(t, Token::Whitespace(_)))
        .collect();
    tokens
        .iter()
        .enumerate()
        .filter_map(|(i, t)| match t {
            Token::Word(w)
                if w.quote_style.is_none() && !matches!(tokens.get(i + 1), Some(Token::LParen)) =>
            {
                Some(w.value.to_ascii_lowercase())
            }
            _ => None,
        })
        .collect()
}

/// Pretty-print `sql` (keywords uppercased, 2-space indent). Comments are kept.
pub fn format_sql(sql: &str) -> String {
    use sqlformat::{FormatOptions, Indent, QueryParams};
    let opts = FormatOptions {
        indent: Indent::Spaces(2),
        uppercase: Some(true),
        lines_between_queries: 1,
        ..Default::default()
    };
    sqlformat::format(sql, &QueryParams::None, &opts)
}

#[cfg(test)]
mod tests {
    use super::*;

    const PG: EngineKind = EngineKind::Postgres;

    fn texts(sql: &str) -> Vec<&str> {
        split_statements(sql, PG)
            .into_iter()
            .map(|r| &sql[r])
            .collect()
    }

    #[test]
    fn formats_select() {
        let out = format_sql("select a,b from t where a=1");
        assert!(out.starts_with("SELECT"));
        assert!(out.contains('\n'));
    }

    #[test]
    fn splits_on_semicolons_outside_quotes_and_comments() {
        let sql = "select ';'; -- a; b\nselect 2;  /* c; d */ select 3";
        assert_eq!(
            texts(sql),
            vec!["select ';'", "-- a; b\nselect 2", "/* c; d */ select 3"]
        );
        assert_eq!(texts(r#"select "a;b" from t; select 2"#).len(), 2);
    }

    #[test]
    fn dollar_quoted_bodies_stay_whole() {
        let sql =
            "create function f() returns int as $$ select 1; select 2; $$ language sql; select 3";
        let parts = texts(sql);
        assert_eq!(parts.len(), 2);
        assert!(parts[0].ends_with("language sql"));
        assert_eq!(parts[1], "select 3");
    }

    #[test]
    fn blank_and_comment_only_stretches_are_not_statements() {
        assert_eq!(texts("select 1;"), vec!["select 1"]);
        assert!(texts("  ;; \n").is_empty());
        assert_eq!(texts("select 1; -- done\n"), vec!["select 1"]);
    }

    #[test]
    fn an_unclosed_quote_runs_to_the_end() {
        let sql = "select 1; select 'unclosed; select 3";
        assert_eq!(texts(sql), vec!["select 1", "select 'unclosed; select 3"]);
    }

    #[test]
    fn multibyte_text_keeps_byte_ranges() {
        let sql = "select 'é;ü' as x; select 2";
        assert_eq!(texts(sql), vec!["select 'é;ü' as x", "select 2"]);
    }

    #[test]
    fn finds_the_statement_under_the_cursor() {
        let sql = "select 1;\nselect 2;\nselect 3";
        let at = |o| &sql[statement_at(sql, PG, o).unwrap()];
        assert_eq!(at(2), "select 1");
        assert_eq!(at(14), "select 2");
        // Cursor just after a closing `;` belongs to the statement it closed.
        assert_eq!(at(9), "select 1");
        assert_eq!(at(sql.len()), "select 3");
    }

    #[test]
    fn keywords_skip_strings_quoted_names_and_comments() {
        let words = keywords(
            "select 'delete' as \"drop\", x -- update\n/* insert */ from t",
            PG,
        );
        assert_eq!(words, ["select", "as", "x", "from", "t"]);
    }
}
