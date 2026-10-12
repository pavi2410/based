use mongodb::error::Result as MongoResult;
use mongodb::options::{ClientOptions, Credential, ResolverConfig};

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

/// Parse a connection URI. For `mongodb+srv://` URIs the system DNS config is tried first; if it
/// can't be used (e.g. macOS scoped IPv6 nameservers like `fe80::1%en0`, which hickory fails to
/// parse), retry with public resolvers.
pub async fn parse_client_options(uri: &str) -> MongoResult<ClientOptions> {
    match ClientOptions::parse(uri).await {
        Err(e) if uri.starts_with("mongodb+srv://") => ClientOptions::parse(uri)
            .resolver_config(ResolverConfig::cloudflare())
            .await
            .map_err(|_| e),
        r => r,
    }
}
