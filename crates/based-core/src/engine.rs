use std::error::Error;
use std::fmt;
use std::str::FromStr;

use serde::{Deserialize, Serialize};

/// A database engine. Serialized as its canonical lowercase name (`postgres`,
/// `mongodb`, `sqlite`, `redis`); the aliases accept the long forms and the
/// PascalCase names older builds wrote.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum EngineKind {
    #[serde(alias = "postgresql", alias = "Postgres")]
    Postgres,
    #[serde(alias = "mongo", alias = "MongoDB")]
    MongoDB,
    #[serde(alias = "SQLite")]
    SQLite,
    #[serde(alias = "Redis")]
    Redis,
}

impl EngineKind {
    pub const ALL: [Self; 4] = [Self::Postgres, Self::MongoDB, Self::SQLite, Self::Redis];

    pub fn short_label(self) -> &'static str {
        match self {
            Self::Postgres => "pg",
            Self::MongoDB => "mg",
            Self::SQLite => "sqlite",
            Self::Redis => "rd",
        }
    }

    /// Canonical storage / config string (`postgres`, `mongodb`, `sqlite`, `redis`).
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Postgres => "postgres",
            Self::MongoDB => "mongodb",
            Self::SQLite => "sqlite",
            Self::Redis => "redis",
        }
    }

    /// Product name for UI (`PostgreSQL`, `MongoDB`, …).
    pub const fn display_name(self) -> &'static str {
        match self {
            Self::Postgres => "PostgreSQL",
            Self::MongoDB => "MongoDB",
            Self::SQLite => "SQLite",
            Self::Redis => "Redis",
        }
    }
}

impl fmt::Display for EngineKind {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

/// An engine name that isn't one of ours.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct UnknownEngine(pub String);

impl fmt::Display for UnknownEngine {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "unknown engine {:?} (expected postgres, mongodb, sqlite or redis)",
            self.0
        )
    }
}

impl Error for UnknownEngine {}

impl FromStr for EngineKind {
    type Err = UnknownEngine;

    /// Case-insensitive; accepts `postgresql` and `mongo`.
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.trim().to_ascii_lowercase().as_str() {
            "postgres" | "postgresql" => Ok(Self::Postgres),
            "mongodb" | "mongo" => Ok(Self::MongoDB),
            "sqlite" => Ok(Self::SQLite),
            "redis" => Ok(Self::Redis),
            _ => Err(UnknownEngine(s.to_string())),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_aliases_case_insensitively() {
        assert_eq!("postgres".parse(), Ok(EngineKind::Postgres));
        assert_eq!("PostgreSQL".parse(), Ok(EngineKind::Postgres));
        assert_eq!("mongo".parse(), Ok(EngineKind::MongoDB));
        assert_eq!("SQLite".parse(), Ok(EngineKind::SQLite));
        assert_eq!("Redis".parse(), Ok(EngineKind::Redis));
        assert!("mongdb".parse::<EngineKind>().is_err());
    }

    #[test]
    fn as_str_round_trips() {
        for engine in EngineKind::ALL {
            assert_eq!(engine.as_str().parse(), Ok(engine));
        }
    }

    #[test]
    fn serde_is_lowercase_and_reads_old_names() {
        assert_eq!(
            serde_json::to_string(&EngineKind::MongoDB).unwrap(),
            r#""mongodb""#
        );
        for (raw, want) in [
            (r#""Postgres""#, EngineKind::Postgres),
            (r#""postgresql""#, EngineKind::Postgres),
            (r#""MongoDB""#, EngineKind::MongoDB),
            (r#""SQLite""#, EngineKind::SQLite),
            (r#""sqlite""#, EngineKind::SQLite),
        ] {
            assert_eq!(serde_json::from_str::<EngineKind>(raw).unwrap(), want);
        }
        assert!(serde_json::from_str::<EngineKind>(r#""mysql""#).is_err());
    }
}
