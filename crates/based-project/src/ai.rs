//! The `[ai]` table of a connection file, read into a single [`ContextLevel`].

use based_core::ContextLevel;
use serde::{Deserialize, Serialize};

/// `[ai] enabled = true, context = "schema"`. AI is off unless `enabled = true`;
/// `context` defaults to `schema`.
#[derive(Debug, Clone, Copy, Deserialize, Serialize)]
pub(crate) struct AiTable {
    #[serde(default)]
    enabled: bool,
    #[serde(default = "default_context")]
    context: ContextLevel,
}

fn default_context() -> ContextLevel {
    ContextLevel::Schema
}

impl AiTable {
    pub(crate) fn level(table: Option<Self>) -> ContextLevel {
        match table {
            Some(t) if t.enabled && t.context != ContextLevel::Off => t.context,
            _ => ContextLevel::Off,
        }
    }

    /// The table to write: none for `Off`.
    pub(crate) fn for_level(level: ContextLevel) -> Option<Self> {
        (level != ContextLevel::Off).then_some(Self {
            enabled: true,
            context: level,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn level(toml: &str) -> ContextLevel {
        AiTable::level(Some(toml::from_str(toml).unwrap()))
    }

    #[test]
    fn every_enabled_context_combination() {
        assert_eq!(
            level("enabled = true\ncontext = \"schema+rows\""),
            ContextLevel::SchemaRows
        );
        assert_eq!(level("enabled = true"), ContextLevel::Schema);
        assert_eq!(
            level("enabled = false\ncontext = \"schema+rows\""),
            ContextLevel::Off
        );
        assert_eq!(level("context = \"schema\""), ContextLevel::Off);
        assert_eq!(AiTable::level(None), ContextLevel::Off);
        assert!(toml::from_str::<AiTable>("context = \"everything\"").is_err());
        assert!(AiTable::for_level(ContextLevel::Off).is_none());
    }
}
