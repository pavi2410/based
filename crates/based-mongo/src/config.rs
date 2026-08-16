use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MongoConfig {
    pub label: String,
    pub uri: String,
    pub database: Option<String>,
    pub auth_source: Option<String>,
}

/// Connection URI. Password is omitted unless requested.
pub fn mongo_uri(config: &MongoConfig, include_password: bool) -> String {
    if include_password {
        return config.uri.clone();
    }
    redact_mongo_password(&config.uri)
}

/// `mongosh` invocation using the same URI as [`mongo_uri`].
pub fn mongosh_command(config: &MongoConfig, include_password: bool) -> String {
    format!("mongosh '{}'", mongo_uri(config, include_password))
}

fn redact_mongo_password(uri: &str) -> String {
    let Some((scheme, rest)) = uri.split_once("://") else {
        return uri.to_string();
    };
    let Some((userinfo, after_at)) = rest.split_once('@') else {
        return uri.to_string();
    };
    let Some((user, _)) = userinfo.split_once(':') else {
        return uri.to_string();
    };
    format!("{scheme}://{user}@{after_at}")
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample(uri: &str) -> MongoConfig {
        MongoConfig {
            label: "local".into(),
            uri: uri.into(),
            database: None,
            auth_source: None,
        }
    }

    #[test]
    fn uri_omits_password_by_default() {
        assert_eq!(
            mongo_uri(
                &sample("mongodb://alice:s3cret@db.example:27017/app"),
                false
            ),
            "mongodb://alice@db.example:27017/app"
        );
    }

    #[test]
    fn uri_includes_password_when_requested() {
        assert_eq!(
            mongo_uri(&sample("mongodb://alice:s3cret@db.example:27017/app"), true),
            "mongodb://alice:s3cret@db.example:27017/app"
        );
    }

    #[test]
    fn uri_without_userinfo_is_unchanged() {
        let uri = "mongodb://localhost:27017/app";
        assert_eq!(mongo_uri(&sample(uri), false), uri);
        assert_eq!(mongo_uri(&sample(uri), true), uri);
    }

    #[test]
    fn uri_user_without_password_is_unchanged() {
        let uri = "mongodb://alice@localhost:27017/app";
        assert_eq!(mongo_uri(&sample(uri), false), uri);
    }

    #[test]
    fn uri_strips_empty_password() {
        assert_eq!(
            mongo_uri(&sample("mongodb://alice:@localhost:27017/app"), false),
            "mongodb://alice@localhost:27017/app"
        );
    }

    #[test]
    fn uri_preserves_srv_and_query() {
        assert_eq!(
            mongo_uri(
                &sample("mongodb+srv://alice:s3cret@cluster.mongodb.net/app?retryWrites=true"),
                false
            ),
            "mongodb+srv://alice@cluster.mongodb.net/app?retryWrites=true"
        );
    }

    #[test]
    fn mongosh_wraps_the_same_uri() {
        let cfg = sample("mongodb://alice:s3cret@db.example:27017/app");
        assert_eq!(
            mongosh_command(&cfg, false),
            "mongosh 'mongodb://alice@db.example:27017/app'"
        );
        assert_eq!(
            mongosh_command(&cfg, true),
            "mongosh 'mongodb://alice:s3cret@db.example:27017/app'"
        );
    }
}
