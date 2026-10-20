use std::mem;

use anyhow::{Result, bail};
use redis::Value;

use crate::client::Conn;

/// Run one command line (shell-style quoting) and return the redis-cli style reply text.
pub async fn exec(conn: &mut Conn, line: &str) -> Result<String> {
    let args = tokenize(line)?;
    let Some((name, rest)) = args.split_first() else {
        bail!("empty command");
    };
    let mut cmd = redis::cmd(name);
    for a in rest {
        cmd.arg(a.as_str());
    }
    let reply: Value = cmd.query_async(conn).await?;
    Ok(format_reply(&reply))
}

/// Commands that should need an explicit second confirmation in the console.
pub fn is_dangerous(line: &str) -> bool {
    let Ok(args) = tokenize(line) else {
        return false;
    };
    let Some(name) = args.first().map(|s| s.to_ascii_uppercase()) else {
        return false;
    };
    match name.as_str() {
        "FLUSHALL" | "FLUSHDB" | "DEBUG" | "SHUTDOWN" => true,
        "KEYS" => args.get(1).is_some_and(|p| p == "*"),
        _ => false,
    }
}

/// Split on whitespace honouring `"..."` (with `\` escapes) and `'...'`.
pub fn tokenize(line: &str) -> Result<Vec<String>> {
    let mut out = Vec::new();
    let mut cur = String::new();
    let mut in_token = false;
    let mut chars = line.chars();
    while let Some(c) = chars.next() {
        match c {
            '"' => {
                in_token = true;
                loop {
                    match chars.next() {
                        Some('"') => break,
                        Some('\\') => match chars.next() {
                            Some('n') => cur.push('\n'),
                            Some('t') => cur.push('\t'),
                            Some('r') => cur.push('\r'),
                            Some(o) => cur.push(o),
                            None => bail!("unterminated string"),
                        },
                        Some(o) => cur.push(o),
                        None => bail!("unterminated string"),
                    }
                }
            }
            '\'' => {
                in_token = true;
                loop {
                    match chars.next() {
                        Some('\'') => break,
                        Some(o) => cur.push(o),
                        None => bail!("unterminated string"),
                    }
                }
            }
            c if c.is_whitespace() => {
                if in_token {
                    out.push(mem::take(&mut cur));
                    in_token = false;
                }
            }
            c => {
                in_token = true;
                cur.push(c);
            }
        }
    }
    if in_token {
        out.push(cur);
    }
    Ok(out)
}

/// redis-cli style rendering of a reply.
pub fn format_reply(v: &Value) -> String {
    let mut out = String::new();
    write_value(&mut out, v, 0);
    out
}

fn write_value(out: &mut String, v: &Value, depth: usize) {
    let pad = "   ".repeat(depth);
    match v {
        Value::Nil => out.push_str("(nil)"),
        Value::Int(n) => out.push_str(&format!("(integer) {n}")),
        Value::Okay => out.push_str("OK"),
        Value::SimpleString(s) => out.push_str(s),
        Value::BulkString(b) => out.push_str(&format!("{:?}", String::from_utf8_lossy(b))),
        Value::Double(d) => out.push_str(&format!("(double) {d}")),
        Value::Boolean(b) => out.push_str(&format!("(boolean) {b}")),
        Value::Array(items) | Value::Set(items) if items.is_empty() => {
            out.push_str("(empty array)")
        }
        Value::Array(items) | Value::Set(items) => {
            for (i, item) in items.iter().enumerate() {
                if i > 0 {
                    out.push('\n');
                    out.push_str(&pad);
                }
                out.push_str(&format!("{}) ", i + 1));
                write_value(out, item, depth + 1);
            }
        }
        Value::Map(pairs) if pairs.is_empty() => out.push_str("(empty map)"),
        Value::Map(pairs) => {
            for (i, (k, val)) in pairs.iter().enumerate() {
                if i > 0 {
                    out.push('\n');
                    out.push_str(&pad);
                }
                out.push_str(&format!("{}# ", i + 1));
                write_value(out, k, depth + 1);
                out.push_str(" => ");
                write_value(out, val, depth + 1);
            }
        }
        Value::VerbatimString { text, .. } => out.push_str(text),
        other => out.push_str(&format!("{other:?}")),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn tokenizes_quotes_and_escapes() {
        assert_eq!(
            tokenize(r#"SET k "a b\n" 'c d' e"#).unwrap(),
            vec!["SET", "k", "a b\n", "c d", "e"]
        );
    }

    #[test]
    fn tokenizes_empty_quoted_arg() {
        assert_eq!(tokenize(r#"SET k """#).unwrap(), vec!["SET", "k", ""]);
    }

    #[test]
    fn unterminated_quote_errors() {
        assert!(tokenize(r#"GET "abc"#).is_err());
    }

    #[test]
    fn flags_dangerous_commands() {
        assert!(is_dangerous("flushall"));
        assert!(is_dangerous("FLUSHDB ASYNC"));
        assert!(is_dangerous("keys *"));
        assert!(!is_dangerous("keys user:*"));
        assert!(!is_dangerous("get k"));
    }

    #[test]
    fn formats_nested_arrays() {
        let v = Value::Array(vec![
            Value::BulkString(b"a".to_vec()),
            Value::Array(vec![Value::Int(1), Value::Nil]),
        ]);
        assert_eq!(format_reply(&v), "1) \"a\"\n2) 1) (integer) 1\n   2) (nil)");
    }

    #[test]
    fn formats_scalars() {
        assert_eq!(format_reply(&Value::Okay), "OK");
        assert_eq!(format_reply(&Value::Nil), "(nil)");
        assert_eq!(format_reply(&Value::Array(vec![])), "(empty array)");
    }
}
