use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum EngineKind {
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
}
