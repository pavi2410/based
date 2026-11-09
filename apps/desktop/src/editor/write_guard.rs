//! Does a SQL text change data or schema? A keyword scan, not a parser: it only
//! decides whether to ask before running on a production connection.

const WRITE_KEYWORDS: &[&str] = &[
    "insert", "update", "delete", "merge", "drop", "alter", "truncate", "create", "grant",
    "revoke", "replace", "vacuum", "reindex",
];

/// `true` when any statement starts with, or (inside a `WITH`) contains, a write keyword.
pub fn changes_data(sql: &str) -> bool {
    words(sql).any(|w| WRITE_KEYWORDS.contains(&w.as_str()))
}

/// What a write does, for a confirmation: the distinct write verbs in order, and a warning
/// when an UPDATE or DELETE has no WHERE clause. Empty for a read.
pub fn write_summary(sql: &str) -> String {
    let all: Vec<String> = words(sql).collect();
    let mut verbs: Vec<String> = Vec::new();
    for w in &all {
        if WRITE_KEYWORDS.contains(&w.as_str()) && !verbs.contains(w) {
            verbs.push(w.clone());
        }
    }
    let unbounded =
        verbs.iter().any(|v| v == "update" || v == "delete") && !all.iter().any(|w| w == "where");
    let mut out = verbs
        .iter()
        .map(|v| v.to_ascii_uppercase())
        .collect::<Vec<_>>()
        .join(", ");
    if unbounded {
        out.push_str(" with no WHERE clause");
    }
    out
}

/// Lower-cased words outside string literals, quoted identifiers and comments.
fn words(sql: &str) -> impl Iterator<Item = String> + '_ {
    let mut out = Vec::new();
    let mut word = String::new();
    let mut chars = sql.chars().peekable();
    let flush = |word: &mut String, out: &mut Vec<String>| {
        if !word.is_empty() {
            out.push(word.to_ascii_lowercase());
            word.clear();
        }
    };
    while let Some(c) = chars.next() {
        match c {
            '\'' | '"' => {
                flush(&mut word, &mut out);
                for n in chars.by_ref() {
                    if n == c {
                        break;
                    }
                }
            }
            '-' if chars.peek() == Some(&'-') => {
                flush(&mut word, &mut out);
                for n in chars.by_ref() {
                    if n == '\n' {
                        break;
                    }
                }
            }
            '/' if chars.peek() == Some(&'*') => {
                flush(&mut word, &mut out);
                chars.next();
                let mut prev = ' ';
                for n in chars.by_ref() {
                    if prev == '*' && n == '/' {
                        break;
                    }
                    prev = n;
                }
            }
            c if c.is_alphanumeric() || c == '_' => word.push(c),
            _ => flush(&mut word, &mut out),
        }
    }
    flush(&mut word, &mut out);
    out.into_iter()
}

#[cfg(test)]
mod tests {
    use super::{changes_data, write_summary};

    #[test]
    fn summary_names_verbs_and_unbounded_writes() {
        assert_eq!(write_summary("SELECT 1"), "");
        assert_eq!(write_summary("DELETE FROM t WHERE id = 1"), "DELETE");
        assert_eq!(
            write_summary("update t set a = 1; drop table x"),
            "UPDATE, DROP with no WHERE clause"
        );
    }

    #[test]
    fn writes_are_detected() {
        assert!(changes_data(
            "DELETE FROM sessions WHERE expires_at < now()"
        ));
        assert!(changes_data("  update users set plan = 'pro'"));
        assert!(changes_data(
            "WITH x AS (SELECT 1) INSERT INTO t SELECT * FROM x"
        ));
        assert!(changes_data("select 1; drop table t"));
    }

    #[test]
    fn reads_and_text_are_not() {
        assert!(!changes_data("SELECT id, email FROM users LIMIT 50"));
        assert!(!changes_data("select 'delete everything' as note"));
        assert!(!changes_data("-- drop table t\nselect 1"));
        assert!(!changes_data("select updated_at, \"delete\" from t"));
        assert!(!changes_data("select /* insert */ 1"));
    }
}
