//! Parse `postgresql://` URIs for the connection form.

use crate::postgres::{PostgresConfig, SslMode};

/// Minimal `postgresql://user:pass@host:port/db?sslmode=prefer` parser.
///
/// A scheme is optional; `user:pass@host:5432/db` is treated like `postgres://…`.
pub(crate) fn parse_postgres_uri(input: &str) -> Option<PostgresConfig> {
    let s = input.trim();
    if s.is_empty() || s.contains(char::is_whitespace) {
        return None;
    }
    let rest = match s.split_once("://") {
        Some((scheme, rest)) => {
            if scheme != "postgresql" && scheme != "postgres" {
                return None;
            }
            rest
        }
        None => s,
    };
    let (credentials, after_at) = match rest.split_once('@') {
        Some((c, h)) => (c, h),
        None => ("", rest),
    };

    let (username, password) = if credentials.is_empty() {
        ("postgres".to_string(), String::new())
    } else if let Some((u, p)) = credentials.split_once(':') {
        (url_decode(u), url_decode(p))
    } else {
        (url_decode(credentials), String::new())
    };

    let (host_part, path_part) = after_at
        .split_once('/')
        .map(|(a, b)| (a, Some(b)))
        .unwrap_or((after_at, None));

    let (host, port) = if let Some((h, p)) = host_part.split_once(':') {
        (h.to_string(), p.parse().unwrap_or(5432u16))
    } else {
        (host_part.to_string(), 5432u16)
    };

    let (database, ssl_mode) = path_part
        .map(parse_path_and_query)
        .unwrap_or(("postgres".to_string(), SslMode::Prefer));

    Some(PostgresConfig {
        label: database.clone(),
        host,
        port,
        database,
        username,
        password,
        ssl_mode,
        ssh: None,
    })
}

fn parse_path_and_query(path_query: &str) -> (String, SslMode) {
    let (path, query) = path_query
        .split_once('?')
        .map(|(p, q)| (p, Some(q)))
        .unwrap_or((path_query, None));
    let db = if path.is_empty() {
        "postgres".to_string()
    } else {
        path.to_string()
    };
    let ssl = query
        .and_then(|q| {
            q.split('&').find_map(|pair| {
                let (k, v) = pair.split_once('=')?;
                if k == "sslmode" {
                    Some(ssl_mode_from_str(v))
                } else {
                    None
                }
            })
        })
        .unwrap_or(SslMode::Prefer);
    (db, ssl)
}

fn ssl_mode_from_str(s: &str) -> SslMode {
    match s.to_ascii_lowercase().as_str() {
        "disable" | "off" => SslMode::Disable,
        "require" => SslMode::Require,
        "verify-ca" => SslMode::VerifyCa,
        "verify-full" => SslMode::VerifyFull,
        _ => SslMode::Prefer,
    }
}

fn url_decode(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    let mut chars = s.chars().peekable();
    while let Some(c) = chars.next() {
        if c == '%' {
            let a = chars.next();
            let b = chars.next();
            if let (Some(a), Some(b)) = (a, b)
                && let Ok(byte) = u8::from_str_radix(&format!("{a}{b}"), 16)
            {
                out.push(byte as char);
                continue;
            }
            out.push(c);
        } else {
            out.push(c);
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::parse_postgres_uri;
    use crate::postgres::SslMode;

    #[test]
    fn parse_full_uri() {
        let cfg = parse_postgres_uri(
            "postgresql://alice:s3cret@db.example:6543/analytics?sslmode=require",
        )
        .expect("uri should parse");
        assert_eq!(cfg.username, "alice");
        assert_eq!(cfg.password, "s3cret");
        assert_eq!(cfg.host, "db.example");
        assert_eq!(cfg.port, 6543);
        assert_eq!(cfg.database, "analytics");
        assert_eq!(cfg.label, "analytics");
        assert!(matches!(cfg.ssl_mode, SslMode::Require));
    }

    #[test]
    fn parse_postgres_scheme_and_defaults() {
        let cfg = parse_postgres_uri("postgres://localhost/").expect("uri should parse");
        assert_eq!(cfg.host, "localhost");
        assert_eq!(cfg.port, 5432);
        assert_eq!(cfg.database, "postgres");
        assert_eq!(cfg.username, "postgres");
        assert!(cfg.password.is_empty());
        assert!(matches!(cfg.ssl_mode, SslMode::Prefer));
    }

    #[test]
    fn parse_url_encoded_password() {
        let cfg = parse_postgres_uri("postgresql://user:p%40ss@localhost/db").unwrap();
        assert_eq!(cfg.password, "p@ss");
    }

    #[test]
    fn parse_rejects_non_postgres_uri() {
        assert!(parse_postgres_uri("mysql://localhost/db").is_none());
        assert!(parse_postgres_uri("not a uri").is_none());
    }

    #[test]
    fn parse_uri_without_scheme() {
        let cfg = parse_postgres_uri("alice:s3cret@db.example:6543/analytics?sslmode=require")
            .expect("schemeless uri should parse");
        assert_eq!(cfg.username, "alice");
        assert_eq!(cfg.password, "s3cret");
        assert_eq!(cfg.host, "db.example");
        assert_eq!(cfg.port, 6543);
        assert_eq!(cfg.database, "analytics");
        assert!(matches!(cfg.ssl_mode, SslMode::Require));
    }

    #[test]
    fn parse_schemeless_host_defaults() {
        let cfg = parse_postgres_uri("localhost/mydb").expect("schemeless host should parse");
        assert_eq!(cfg.host, "localhost");
        assert_eq!(cfg.port, 5432);
        assert_eq!(cfg.database, "mydb");
        assert_eq!(cfg.username, "postgres");
        assert!(cfg.password.is_empty());
        assert!(matches!(cfg.ssl_mode, SslMode::Prefer));
    }
}
