//! Pure helpers for the unified New connection form.

use std::path::Path;

use crate::connection::{ConnectionConfig, ConnectionId, ConnectionOrigin};
use crate::postgres::SslMode;

/// Where a blank Name field should take its save label from.
#[derive(Debug, Clone, Copy)]
pub enum SaveLabelSource<'a> {
    Postgres {
        host: &'a str,
        database: &'a str,
    },
    Mongo {
        uri: &'a str,
        database: Option<&'a str>,
    },
    Sqlite {
        path: &'a str,
    },
}

/// Name used when saving. Typed text wins; otherwise host / file stem, never an engine word.
pub fn wizard_save_label(name: &str, source: SaveLabelSource<'_>) -> String {
    let name = name.trim();
    if !name.is_empty() {
        return name.to_string();
    }
    match source {
        SaveLabelSource::Postgres { host, database } => first_nonempty(&[host, database]),
        SaveLabelSource::Mongo { uri, database } => {
            let host = mongo_host(uri);
            first_nonempty(&[host.as_deref().unwrap_or(""), database.unwrap_or("")])
        }
        SaveLabelSource::Sqlite { path } => sqlite_stem(path),
    }
}

pub fn save_label_from_config(name: &str, config: &ConnectionConfig) -> String {
    match config {
        ConnectionConfig::Postgres(c) => wizard_save_label(
            name,
            SaveLabelSource::Postgres {
                host: &c.host,
                database: &c.database,
            },
        ),
        ConnectionConfig::MongoDB(c) => wizard_save_label(
            name,
            SaveLabelSource::Mongo {
                uri: &c.uri,
                database: c.database.as_deref(),
            },
        ),
        ConnectionConfig::SQLite(c) => wizard_save_label(
            name,
            SaveLabelSource::Sqlite {
                path: &c.path.to_string_lossy(),
            },
        ),
    }
}

/// SSL off → Disable. On with no/loose mode → Require; keep verify-ca / verify-full.
pub fn ssl_mode_from_toggle(enabled: bool, selected: Option<SslMode>) -> SslMode {
    if !enabled {
        return SslMode::Disable;
    }
    match selected {
        Some(SslMode::VerifyCa) => SslMode::VerifyCa,
        Some(SslMode::VerifyFull) => SslMode::VerifyFull,
        _ => SslMode::Require,
    }
}

pub fn ssl_toggle_enabled(mode: SslMode) -> bool {
    !matches!(mode, SslMode::Disable)
}

/// Reuse a saved id; otherwise mint an ephemeral live session id.
pub fn wizard_session_id(saved_id: Option<&ConnectionId>, unsaved_key: &str) -> ConnectionId {
    saved_id
        .cloned()
        .unwrap_or_else(|| ConnectionId::unsaved(unsaved_key))
}

pub fn saved_id_for_destination(origin: ConnectionOrigin, relative_id: &str) -> ConnectionId {
    match origin {
        ConnectionOrigin::Personal => ConnectionId::personal(relative_id),
        ConnectionOrigin::Project => ConnectionId::from_key(relative_id),
    }
}

/// Add a trimmed tag. Empty and duplicate names are ignored (case-sensitive).
pub fn add_wizard_tag(tags: &mut Vec<String>, raw: &str) -> bool {
    let tag = raw.trim();
    if tag.is_empty() || tags.iter().any(|existing| existing == tag) {
        return false;
    }
    tags.push(tag.to_string());
    true
}

pub fn remove_wizard_tag(tags: &mut Vec<String>, tag: &str) {
    tags.retain(|existing| existing != tag);
}

fn first_nonempty(parts: &[&str]) -> String {
    parts
        .iter()
        .map(|s| s.trim())
        .find(|s| !s.is_empty())
        .unwrap_or("connection")
        .to_string()
}

fn mongo_host(uri: &str) -> Option<String> {
    let rest = uri.split_once("://").map(|(_, r)| r).unwrap_or(uri).trim();
    if rest.is_empty() {
        return None;
    }
    let after_at = rest.split_once('@').map(|(_, h)| h).unwrap_or(rest);
    let host = after_at.split(['/', '?', ',']).next().unwrap_or(after_at);
    let host = host.rsplit_once(':').map(|(h, _)| h).unwrap_or(host).trim();
    if host.is_empty() {
        None
    } else {
        Some(host.to_string())
    }
}

fn sqlite_stem(path: &str) -> String {
    Path::new(path)
        .file_stem()
        .and_then(|s| s.to_str())
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .unwrap_or("connection")
        .to_string()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::postgres::PostgresConfig;
    use crate::sqlite::SqliteConfig;
    use std::path::PathBuf;

    #[test]
    fn save_label_prefers_typed_name() {
        let label = wizard_save_label(
            "  Analytics  ",
            SaveLabelSource::Postgres {
                host: "db.example",
                database: "postgres",
            },
        );
        assert_eq!(label, "Analytics");
    }

    #[test]
    fn empty_name_uses_postgres_host_not_engine_word() {
        let label = wizard_save_label(
            "",
            SaveLabelSource::Postgres {
                host: "db.example",
                database: "postgres",
            },
        );
        assert_eq!(label, "db.example");
        assert_ne!(label, "PostgreSQL");
    }

    #[test]
    fn empty_name_and_host_uses_database() {
        let label = wizard_save_label(
            "   ",
            SaveLabelSource::Postgres {
                host: "",
                database: "analytics",
            },
        );
        assert_eq!(label, "analytics");
    }

    #[test]
    fn empty_postgres_fields_fall_back_to_connection() {
        let label = wizard_save_label(
            "",
            SaveLabelSource::Postgres {
                host: "",
                database: "",
            },
        );
        assert_eq!(label, "connection");
    }

    #[test]
    fn empty_name_uses_sqlite_file_stem() {
        let label = wizard_save_label(
            "",
            SaveLabelSource::Sqlite {
                path: "/tmp/northwind.db",
            },
        );
        assert_eq!(label, "northwind");
    }

    #[test]
    fn empty_name_uses_mongo_host_from_uri() {
        let label = wizard_save_label(
            "",
            SaveLabelSource::Mongo {
                uri: "mongodb://alice:s3cret@cluster.example:27017/app",
                database: Some("app"),
            },
        );
        assert_eq!(label, "cluster.example");
    }

    #[test]
    fn save_label_from_config_uses_sqlite_path() {
        let config = ConnectionConfig::SQLite(SqliteConfig {
            label: String::new(),
            path: PathBuf::from("/data/shop.sqlite"),
            read_only: false,
            pragma: None,
        });
        assert_eq!(save_label_from_config("", &config), "shop");
    }

    #[test]
    fn ssl_toggle_off_is_disable() {
        assert!(matches!(
            ssl_mode_from_toggle(false, Some(SslMode::Require)),
            SslMode::Disable
        ));
        assert!(!ssl_toggle_enabled(SslMode::Disable));
    }

    #[test]
    fn ssl_toggle_on_defaults_to_require() {
        assert!(matches!(ssl_mode_from_toggle(true, None), SslMode::Require));
        assert!(matches!(
            ssl_mode_from_toggle(true, Some(SslMode::Prefer)),
            SslMode::Require
        ));
        assert!(ssl_toggle_enabled(SslMode::Require));
        assert!(ssl_toggle_enabled(SslMode::Prefer));
    }

    #[test]
    fn ssl_toggle_on_keeps_verify_modes() {
        assert!(matches!(
            ssl_mode_from_toggle(true, Some(SslMode::VerifyCa)),
            SslMode::VerifyCa
        ));
        assert!(matches!(
            ssl_mode_from_toggle(true, Some(SslMode::VerifyFull)),
            SslMode::VerifyFull
        ));
    }

    #[test]
    fn session_id_reuses_saved_and_mints_unsaved() {
        let saved = ConnectionId::personal("analytics");
        assert_eq!(wizard_session_id(Some(&saved), "ignored"), saved);
        let unsaved = wizard_session_id(None, "aabbccdd");
        assert!(unsaved.is_unsaved());
        assert_eq!(unsaved.0, "unsaved:aabbccdd");
    }

    #[test]
    fn saved_id_matches_destination_origin() {
        assert_eq!(
            saved_id_for_destination(ConnectionOrigin::Personal, "analytics").0,
            "user:analytics"
        );
        assert_eq!(
            saved_id_for_destination(ConnectionOrigin::Project, "local/pg").0,
            "local/pg"
        );
    }

    #[test]
    fn with_label_overrides_engine_default() {
        let config = ConnectionConfig::Postgres(PostgresConfig {
            label: String::new(),
            host: "db.example".into(),
            port: 5432,
            database: "postgres".into(),
            username: "postgres".into(),
            password: String::new(),
            ssl_mode: SslMode::Disable,
        })
        .with_label("Analytics".into());
        assert_eq!(config.label(), "Analytics");
    }

    #[test]
    fn add_wizard_tag_trims_and_skips_empty_or_duplicate() {
        let mut tags = Vec::new();
        assert!(add_wizard_tag(&mut tags, "  local  "));
        assert_eq!(tags, vec!["local"]);
        assert!(!add_wizard_tag(&mut tags, "local"));
        assert!(!add_wizard_tag(&mut tags, "   "));
        assert!(add_wizard_tag(&mut tags, "dev"));
        assert_eq!(tags, vec!["local", "dev"]);
    }

    #[test]
    fn remove_wizard_tag_drops_matching_name() {
        let mut tags = vec!["local".into(), "dev".into()];
        remove_wizard_tag(&mut tags, "local");
        assert_eq!(tags, vec!["dev"]);
        remove_wizard_tag(&mut tags, "missing");
        assert_eq!(tags, vec!["dev"]);
    }
}
