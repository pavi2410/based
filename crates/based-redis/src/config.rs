use serde::{Deserialize, Serialize};

/// A single-node Redis connection. The URL carries host, credentials, TLS (`rediss://`) and DB index.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RedisConfig {
    pub label: String,
    pub url: String,
}

/// Connection URL. Password is omitted unless requested.
pub fn redis_url(config: &RedisConfig, include_password: bool) -> String {
    if include_password {
        return config.url.clone();
    }
    redact_password(&config.url)
}

/// `redis-cli` invocation using the same URL as [`redis_url`].
pub fn redis_cli_command(config: &RedisConfig, include_password: bool) -> String {
    format!("redis-cli -u '{}'", redis_url(config, include_password))
}

/// Byte range of the `/db` path segment in `url`, if present.
fn db_path_range(url: &str) -> (usize, usize) {
    let query = url.find('?').unwrap_or(url.len());
    let auth_start = url.find("://").map_or(0, |i| i + 3);
    let host_start = url[auth_start..query]
        .rfind('@')
        .map_or(auth_start, |i| auth_start + i + 1);
    let path_start = url[host_start..query]
        .find('/')
        .map_or(query, |i| host_start + i);
    (path_start, query)
}

/// Database index encoded in the URL path (`redis://h:6379/2` -> 2); defaults to 0.
pub fn url_db(url: &str) -> u32 {
    let (start, end) = db_path_range(url);
    url[start..end].trim_start_matches('/').parse().unwrap_or(0)
}

/// Same URL pointing at database `db`.
pub fn url_with_db(url: &str, db: u32) -> String {
    let (start, end) = db_path_range(url);
    format!("{}/{db}{}", &url[..start], &url[end..])
}

fn redact_password(url: &str) -> String {
    let Some((scheme, rest)) = url.split_once("://") else {
        return url.to_string();
    };
    let Some((userinfo, after_at)) = rest.split_once('@') else {
        return url.to_string();
    };
    let Some((user, _)) = userinfo.split_once(':') else {
        return url.to_string();
    };
    if user.is_empty() {
        format!("{scheme}://{after_at}")
    } else {
        format!("{scheme}://{user}@{after_at}")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn cfg(url: &str) -> RedisConfig {
        RedisConfig {
            label: "local".into(),
            url: url.into(),
        }
    }

    #[test]
    fn reads_and_sets_db() {
        assert_eq!(url_db("redis://h:6379"), 0);
        assert_eq!(url_db("redis://u:p@h:6379/3?x=1"), 3);
        assert_eq!(url_with_db("redis://h:6379", 5), "redis://h:6379/5");
        assert_eq!(
            url_with_db("rediss://u:p@h:6379/3?x=1", 7),
            "rediss://u:p@h:6379/7?x=1"
        );
    }

    #[test]
    fn redacts_user_password() {
        assert_eq!(
            redis_url(&cfg("redis://alice:s3cret@h:6379/2"), false),
            "redis://alice@h:6379/2"
        );
    }

    #[test]
    fn redacts_password_only_form() {
        assert_eq!(
            redis_url(&cfg("rediss://:s3cret@h:6380"), false),
            "rediss://h:6380"
        );
    }

    #[test]
    fn keeps_password_when_requested() {
        let url = "redis://alice:s3cret@h:6379";
        assert_eq!(redis_url(&cfg(url), true), url);
    }

    #[test]
    fn url_without_credentials_is_unchanged() {
        assert_eq!(redis_url(&cfg("redis://h:6379"), false), "redis://h:6379");
    }

    #[test]
    fn cli_command_wraps_url() {
        assert_eq!(
            redis_cli_command(&cfg("redis://h:6379"), false),
            "redis-cli -u 'redis://h:6379'"
        );
    }
}
