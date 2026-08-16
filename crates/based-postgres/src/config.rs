use serde::{Deserialize, Serialize};
use sqlx::postgres::{PgConnectOptions, PgSslMode};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PostgresConfig {
    pub label: String,
    pub host: String,
    pub port: u16,
    pub database: String,
    pub username: String,
    pub password: String,
    pub ssl_mode: SslMode,
}

#[derive(Debug, Clone, Copy, Default, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SslMode {
    #[default]
    Prefer,
    Require,
    Disable,
    VerifyCa,
    VerifyFull,
}

pub fn pg_ssl_mode(m: SslMode) -> PgSslMode {
    match m {
        SslMode::Disable => PgSslMode::Disable,
        SslMode::Prefer => PgSslMode::Prefer,
        SslMode::Require => PgSslMode::Require,
        SslMode::VerifyCa => PgSslMode::VerifyCa,
        SslMode::VerifyFull => PgSslMode::VerifyFull,
    }
}

pub fn pg_connect_options(config: &PostgresConfig) -> PgConnectOptions {
    PgConnectOptions::new()
        .host(&config.host)
        .port(config.port)
        .username(&config.username)
        .password(&config.password)
        .database(&config.database)
        .ssl_mode(pg_ssl_mode(config.ssl_mode))
}

/// libpq URI (`postgresql://…`). Password is omitted unless requested and non-empty.
pub fn postgres_uri(config: &PostgresConfig, include_password: bool) -> String {
    let user = percent_encode(&config.username);
    let userinfo = if include_password && !config.password.is_empty() {
        format!("{user}:{}", percent_encode(&config.password))
    } else {
        user
    };
    let host = format_host(&config.host);
    let database = percent_encode(&config.database);
    format!(
        "postgresql://{userinfo}@{host}:{port}/{database}?sslmode={sslmode}",
        port = config.port,
        sslmode = sslmode_label(config.ssl_mode),
    )
}

/// `psql` invocation using the same URI as [`postgres_uri`].
pub fn psql_command(config: &PostgresConfig, include_password: bool) -> String {
    format!("psql '{}'", postgres_uri(config, include_password))
}

fn sslmode_label(mode: SslMode) -> &'static str {
    match mode {
        SslMode::Disable => "disable",
        SslMode::Prefer => "prefer",
        SslMode::Require => "require",
        SslMode::VerifyCa => "verify-ca",
        SslMode::VerifyFull => "verify-full",
    }
}

fn format_host(host: &str) -> String {
    if host.contains(':') && !host.starts_with('[') {
        format!("[{host}]")
    } else {
        host.to_string()
    }
}

fn percent_encode(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    for byte in s.as_bytes() {
        match byte {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'.' | b'_' | b'~' => {
                out.push(*byte as char);
            }
            _ => out.push_str(&format!("%{byte:02X}")),
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample() -> PostgresConfig {
        PostgresConfig {
            label: "analytics".into(),
            host: "db.example".into(),
            port: 6543,
            database: "analytics".into(),
            username: "alice".into(),
            password: "s3cret".into(),
            ssl_mode: SslMode::Require,
        }
    }

    #[test]
    fn uri_omits_password_by_default() {
        assert_eq!(
            postgres_uri(&sample(), false),
            "postgresql://alice@db.example:6543/analytics?sslmode=require"
        );
    }

    #[test]
    fn uri_includes_password_when_requested() {
        assert_eq!(
            postgres_uri(&sample(), true),
            "postgresql://alice:s3cret@db.example:6543/analytics?sslmode=require"
        );
    }

    #[test]
    fn uri_with_password_omits_empty_password() {
        let mut cfg = sample();
        cfg.password.clear();
        assert_eq!(postgres_uri(&cfg, true), postgres_uri(&cfg, false));
        assert_eq!(
            postgres_uri(&cfg, true),
            "postgresql://alice@db.example:6543/analytics?sslmode=require"
        );
    }

    #[test]
    fn uri_percent_encodes_userinfo_and_database() {
        let cfg = PostgresConfig {
            label: "odd".into(),
            host: "localhost".into(),
            port: 5432,
            database: "sales q".into(),
            username: "al ice".into(),
            password: "p@ss:w/rd".into(),
            ssl_mode: SslMode::Prefer,
        };
        assert_eq!(
            postgres_uri(&cfg, true),
            "postgresql://al%20ice:p%40ss%3Aw%2Frd@localhost:5432/sales%20q?sslmode=prefer"
        );
    }

    #[test]
    fn uri_wraps_ipv6_host() {
        let mut cfg = sample();
        cfg.host = "::1".into();
        cfg.port = 5432;
        cfg.ssl_mode = SslMode::Disable;
        assert_eq!(
            postgres_uri(&cfg, false),
            "postgresql://alice@[::1]:5432/analytics?sslmode=disable"
        );
    }

    #[test]
    fn uri_maps_all_ssl_modes() {
        let mut cfg = sample();
        cfg.ssl_mode = SslMode::Disable;
        assert!(postgres_uri(&cfg, false).ends_with("sslmode=disable"));
        cfg.ssl_mode = SslMode::Prefer;
        assert!(postgres_uri(&cfg, false).ends_with("sslmode=prefer"));
        cfg.ssl_mode = SslMode::Require;
        assert!(postgres_uri(&cfg, false).ends_with("sslmode=require"));
        cfg.ssl_mode = SslMode::VerifyCa;
        assert!(postgres_uri(&cfg, false).ends_with("sslmode=verify-ca"));
        cfg.ssl_mode = SslMode::VerifyFull;
        assert!(postgres_uri(&cfg, false).ends_with("sslmode=verify-full"));
    }

    #[test]
    fn psql_wraps_the_same_uri() {
        assert_eq!(
            psql_command(&sample(), false),
            "psql 'postgresql://alice@db.example:6543/analytics?sslmode=require'"
        );
        assert_eq!(
            psql_command(&sample(), true),
            "psql 'postgresql://alice:s3cret@db.example:6543/analytics?sslmode=require'"
        );
    }
}
