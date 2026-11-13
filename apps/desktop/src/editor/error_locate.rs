//! Find where in the SQL a database error points, from the error text alone.
//!
//! Servers name the offending token (`syntax error at or near "SELEC"`); the
//! first occurrence of that token in the statement is the best guess.

use std::ops::Range;

/// Byte range of the token an error message names, if it names one.
pub fn locate_error(sql: &str, error: &str) -> Option<Range<usize>> {
    let token = quoted_after(error, "near \"")?;
    if token.is_empty() {
        return None;
    }
    let start = find_token(sql, token)?;
    Some(start..start + token.len())
}

/// The token starting at byte `offset`: a run of word characters, else one character.
pub fn token_at(sql: &str, offset: usize) -> Option<Range<usize>> {
    let rest = sql.get(offset..)?;
    let is_word = |c: char| c.is_alphanumeric() || c == '_';
    let first = rest.chars().next()?;
    let len = if is_word(first) {
        rest.find(|c: char| !is_word(c)).unwrap_or(rest.len())
    } else {
        first.len_utf8()
    };
    Some(offset..offset + len)
}

fn quoted_after<'a>(text: &'a str, marker: &str) -> Option<&'a str> {
    let rest = &text[text.find(marker)? + marker.len()..];
    Some(&rest[..rest.find('"')?])
}

/// First match that is not in the middle of a longer identifier.
fn find_token(sql: &str, token: &str) -> Option<usize> {
    let is_word = |c: char| c.is_alphanumeric() || c == '_';
    sql.match_indices(token).map(|(i, _)| i).find(|&i| {
        let before = sql[..i].chars().next_back();
        let after = sql[i + token.len()..].chars().next();
        let first_is_word = token.chars().next().is_some_and(is_word);
        let last_is_word = token.chars().next_back().is_some_and(is_word);
        !(first_is_word && before.is_some_and(is_word))
            && !(last_is_word && after.is_some_and(is_word))
    })
}

/// 1-based line and column of byte `offset`.
pub fn line_col(sql: &str, offset: usize) -> (usize, usize) {
    let offset = offset.min(sql.len());
    let before = &sql[..offset];
    let line = before.matches('\n').count() + 1;
    let col = before.rsplit('\n').next().map_or(0, |l| l.chars().count()) + 1;
    (line, col)
}

#[cfg(test)]
mod tests {
    use super::{line_col, locate_error, token_at};

    #[test]
    fn token_at_takes_a_word_or_one_character() {
        let sql = "select * from nope_é";
        assert_eq!(token_at(sql, 14), Some(14..sql.len()));
        assert_eq!(token_at(sql, 7), Some(7..8));
        assert_eq!(token_at(sql, sql.len()), None);
    }

    #[test]
    fn finds_the_token_a_postgres_error_names() {
        let sql = "-- c\nSELEC id FROM users";
        let r = locate_error(sql, "syntax error at or near \"SELEC\"").unwrap();
        assert_eq!(&sql[r.clone()], "SELEC");
        assert_eq!(line_col(sql, r.start), (2, 1));
    }

    #[test]
    fn skips_matches_inside_longer_words() {
        let sql = "SELECT selection, sel FROM t";
        let r = locate_error(sql, "near \"sel\": syntax error").unwrap();
        assert_eq!(&sql[r.clone()], "sel");
        assert_eq!(r.start, sql.rfind("sel").unwrap());
    }

    #[test]
    fn no_token_means_no_location() {
        assert!(locate_error("select 1", "connection refused").is_none());
        assert!(locate_error("select 1", "near \"zzz\" error").is_none());
    }
}
