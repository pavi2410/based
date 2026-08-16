use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
pub enum EngineKind {
    #[default]
    Postgres,
    MongoDB,
    SQLite,
}

impl EngineKind {
    pub fn short_label(self) -> &'static str {
        match self {
            Self::Postgres => "pg",
            Self::MongoDB => "mg",
            Self::SQLite => "sqlite",
        }
    }

    /// Canonical storage / config string (`postgres`, `mongodb`, `sqlite`).
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Postgres => "postgres",
            Self::MongoDB => "mongodb",
            Self::SQLite => "sqlite",
        }
    }

    /// Parse engine labels; unknown values fall back to Postgres.
    ///
    /// Accepts aliases: `postgresql`, `mongo`.
    pub fn from_str_lossy(s: &str) -> Self {
        match s.to_ascii_lowercase().as_str() {
            "mongodb" | "mongo" => Self::MongoDB,
            "sqlite" => Self::SQLite,
            "postgres" | "postgresql" => Self::Postgres,
            _ => Self::Postgres,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::EngineKind;

    #[test]
    fn from_str_lossy_aliases() {
        assert_eq!(EngineKind::from_str_lossy("postgres"), EngineKind::Postgres);
        assert_eq!(
            EngineKind::from_str_lossy("postgresql"),
            EngineKind::Postgres
        );
        assert_eq!(EngineKind::from_str_lossy("mongodb"), EngineKind::MongoDB);
        assert_eq!(EngineKind::from_str_lossy("mongo"), EngineKind::MongoDB);
        assert_eq!(EngineKind::from_str_lossy("sqlite"), EngineKind::SQLite);
        assert_eq!(EngineKind::from_str_lossy("SQLite"), EngineKind::SQLite);
        assert_eq!(EngineKind::from_str_lossy("nope"), EngineKind::Postgres);
    }

    #[test]
    fn as_str_round_trips() {
        for engine in [
            EngineKind::Postgres,
            EngineKind::MongoDB,
            EngineKind::SQLite,
        ] {
            assert_eq!(EngineKind::from_str_lossy(engine.as_str()), engine);
        }
    }

    #[test]
    fn default_is_postgres() {
        assert_eq!(EngineKind::default(), EngineKind::Postgres);
    }
}
