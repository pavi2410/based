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
