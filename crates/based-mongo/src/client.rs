use mongodb::options::{ClientOptions, Credential};

pub fn apply_auth_source(opts: &mut ClientOptions, src: &str) {
    match opts.credential.as_mut() {
        Some(cred) => {
            cred.source = Some(src.to_string());
        }
        None => {
            opts.credential = Some(Credential::builder().source(Some(src.to_string())).build());
        }
    }
}

/// Pick the database name for a live connection from config + parsed URI options.
pub fn resolve_database_name(config: &super::MongoConfig, opts: &ClientOptions) -> String {
    config
        .database
        .clone()
        .or_else(|| opts.default_database.as_ref().map(|s| s.to_string()))
        .unwrap_or_else(|| "test".to_string())
}

/// Database used for connectivity tests (ping/buildInfo).
pub fn test_database_name(config: &super::MongoConfig) -> &str {
    config.database.as_deref().unwrap_or("admin")
}
