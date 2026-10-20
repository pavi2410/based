//! Lightweight SQL scanning: which tables (and aliases) a statement references.

/// A relation named in `FROM` / `JOIN` / `UPDATE` / `INTO`.
#[derive(Debug, PartialEq, Eq)]
pub struct ScopeTable {
    /// Possibly quoted/dotted, exactly as written.
    pub name: String,
    pub alias: Option<String>,
}

#[derive(Debug, PartialEq, Eq)]
enum Tok<'a> {
    Word(&'a str),
    Punct(char),
}

const NOT_ALIAS: &[&str] = &[
    "WHERE",
    "ON",
    "JOIN",
    "INNER",
    "LEFT",
    "RIGHT",
    "FULL",
    "CROSS",
    "OUTER",
    "NATURAL",
    "GROUP",
    "ORDER",
    "LIMIT",
    "OFFSET",
    "HAVING",
    "UNION",
    "USING",
    "SET",
    "VALUES",
    "SELECT",
    "RETURNING",
    "WINDOW",
    "LATERAL",
    "EXCEPT",
    "INTERSECT",
    "AS",
    "FROM",
];

fn tokens(s: &str) -> Vec<Tok<'_>> {
    let mut out = Vec::new();
    let mut chars = s.char_indices().peekable();
    while let Some((i, c)) = chars.next() {
        if c.is_whitespace() {
            continue;
        }
        if c == '-' && s[i..].starts_with("--") {
            while chars.next_if(|&(_, c)| c != '\n').is_some() {}
            continue;
        }
        if c == '\'' {
            for (_, c) in chars.by_ref() {
                if c == '\'' {
                    break;
                }
            }
            out.push(Tok::Punct('\''));
            continue;
        }
        if c.is_alphanumeric() || matches!(c, '_' | '"' | '$') {
            let mut inq = c == '"';
            let mut end = i + c.len_utf8();
            while let Some(&(j, n)) = chars.peek() {
                let ident = n.is_alphanumeric() || matches!(n, '_' | '$' | '.' | '"');
                if !(inq || ident) {
                    break;
                }
                if n == '"' {
                    inq = !inq;
                }
                end = j + n.len_utf8();
                chars.next();
            }
            out.push(Tok::Word(&s[i..end]));
        } else {
            out.push(Tok::Punct(c));
        }
    }
    out
}

fn is_kw(w: &str, set: &[&str]) -> bool {
    set.iter().any(|k| k.eq_ignore_ascii_case(w))
}

/// Tables referenced by `stmt`, in order of appearance.
pub fn tables_in_scope(stmt: &str) -> Vec<ScopeTable> {
    let toks = tokens(stmt);
    let mut out = Vec::new();
    let mut i = 0;
    while i < toks.len() {
        let Tok::Word(w) = toks[i] else {
            i += 1;
            continue;
        };
        i += 1;
        if !is_kw(w, &["FROM", "JOIN", "UPDATE", "INTO"]) {
            continue;
        }
        let from = w.eq_ignore_ascii_case("FROM");
        loop {
            let Some(&Tok::Word(name)) = toks.get(i) else {
                break;
            };
            if is_kw(name, NOT_ALIAS) {
                break;
            }
            i += 1;
            let mut alias = None;
            if let Some(&Tok::Word(a)) = toks.get(i)
                && a.eq_ignore_ascii_case("AS")
            {
                i += 1;
            }
            if let Some(&Tok::Word(a)) = toks.get(i)
                && !is_kw(a, NOT_ALIAS)
                && !is_kw(a, &["FROM", "JOIN", "UPDATE", "INTO"])
            {
                alias = Some(a.to_string());
                i += 1;
            }
            out.push(ScopeTable {
                name: name.to_string(),
                alias,
            });
            if from && toks.get(i) == Some(&Tok::Punct(',')) {
                i += 1;
            } else {
                break;
            }
        }
    }
    out
}

/// Last word before the end of `prefix`, uppercased (`None` if the prefix ends in punctuation).
pub fn previous_word(prefix: &str) -> Option<String> {
    match tokens(prefix).last() {
        Some(Tok::Word(w)) => Some(w.to_uppercase()),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn names(sql: &str) -> Vec<(String, Option<String>)> {
        tables_in_scope(sql)
            .into_iter()
            .map(|t| (t.name, t.alias))
            .collect()
    }

    #[test]
    fn finds_tables_and_aliases() {
        let got = names("SELECT * FROM users u JOIN \"my orders\" AS o ON u.id = o.uid WHERE x");
        assert_eq!(
            got,
            vec![
                ("users".into(), Some("u".into())),
                ("\"my orders\"".into(), Some("o".into()))
            ]
        );
    }

    #[test]
    fn comma_list_and_no_alias() {
        let got = names("select 1 from a, b where 1");
        assert_eq!(got, vec![("a".into(), None), ("b".into(), None)]);
    }

    #[test]
    fn previous_word_is_uppercased() {
        assert_eq!(previous_word("select * from "), Some("FROM".into()));
        assert_eq!(previous_word("select a,"), None);
    }
}
