//! Split SQL text into statements and find the one under the cursor.

use std::ops::Range;

/// Byte ranges of each non-empty statement, trimmed, without the closing `;`.
///
/// Semicolons inside quotes, comments and dollar-quoted bodies do not split.
pub fn split_statements(sql: &str) -> Vec<Range<usize>> {
    let bytes = sql.as_bytes();
    let mut out = Vec::new();
    let mut start = 0;
    let mut i = 0;
    let push = |out: &mut Vec<Range<usize>>, from: usize, to: usize| {
        let seg = &sql[from..to];
        let lead = seg.len() - seg.trim_start().len();
        let trimmed = seg.trim();
        if !trimmed.is_empty() {
            out.push(from + lead..from + lead + trimmed.len());
        }
    };
    while i < bytes.len() {
        match bytes[i] {
            b'\'' | b'"' => {
                let q = bytes[i];
                i += 1;
                while i < bytes.len() && bytes[i] != q {
                    i += 1;
                }
                i += 1;
            }
            b'-' if bytes.get(i + 1) == Some(&b'-') => {
                while i < bytes.len() && bytes[i] != b'\n' {
                    i += 1;
                }
            }
            b'/' if bytes.get(i + 1) == Some(&b'*') => {
                i += 2;
                while i + 1 < bytes.len() && !(bytes[i] == b'*' && bytes[i + 1] == b'/') {
                    i += 1;
                }
                i += 2;
            }
            b'$' => {
                // $tag$ ... $tag$
                let tag_end = sql[i + 1..].find('$').map(|n| i + 1 + n);
                match tag_end {
                    Some(end)
                        if sql[i + 1..end]
                            .chars()
                            .all(|c| c.is_alphanumeric() || c == '_') =>
                    {
                        let tag = &sql[i..=end];
                        match sql[end + 1..].find(tag) {
                            Some(n) => i = end + 1 + n + tag.len(),
                            None => i = bytes.len(),
                        }
                    }
                    _ => i += 1,
                }
            }
            b';' => {
                push(&mut out, start, i);
                i += 1;
                start = i;
            }
            _ => i += 1,
        }
    }
    push(&mut out, start, bytes.len().min(sql.len()));
    out
}

/// The statement containing `offset`, or the closest one before it (cursor in a gap or
/// after a trailing `;`). The first statement when the cursor is before all of them.
pub fn statement_at(sql: &str, offset: usize) -> Option<Range<usize>> {
    let stmts = split_statements(sql);
    stmts
        .iter()
        .find(|r| r.start <= offset && offset <= r.end + 1)
        .or_else(|| stmts.iter().rev().find(|r| r.end < offset))
        .or_else(|| stmts.first())
        .cloned()
}

#[cfg(test)]
mod tests {
    use super::{split_statements, statement_at};

    fn texts(sql: &str) -> Vec<&str> {
        split_statements(sql).into_iter().map(|r| &sql[r]).collect()
    }

    #[test]
    fn splits_on_semicolons_outside_quotes_and_comments() {
        let sql = "select ';'; -- a; b\nselect 2;  /* c; d */ select 3";
        assert_eq!(
            texts(sql),
            vec!["select ';'", "-- a; b\nselect 2", "/* c; d */ select 3"]
        );
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
    fn single_statement_and_blank_text() {
        assert_eq!(texts("select 1;"), vec!["select 1"]);
        assert!(texts("  ;; \n").is_empty());
    }

    #[test]
    fn finds_the_statement_under_the_cursor() {
        let sql = "select 1;\nselect 2;\nselect 3";
        assert_eq!(&sql[statement_at(sql, 2).unwrap()], "select 1");
        assert_eq!(&sql[statement_at(sql, 14).unwrap()], "select 2");
        // Cursor just after a closing `;` belongs to the statement it closed.
        assert_eq!(&sql[statement_at(sql, 9).unwrap()], "select 1");
        assert_eq!(&sql[statement_at(sql, sql.len()).unwrap()], "select 3");
    }
}
