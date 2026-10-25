use serde::{Deserialize, Serialize};

/// How much of a connection an AI assistant may see.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Deserialize, Serialize)]
pub enum AiContext {
    Off,
    #[default]
    #[serde(rename = "schema")]
    Schema,
    #[serde(rename = "schema+rows")]
    SchemaRows,
}

/// Per-connection `[ai]` table. AI is off unless `enabled = true`.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Deserialize, Serialize)]
pub struct AiSettings {
    #[serde(default)]
    pub enabled: bool,
    #[serde(default)]
    pub context: AiContext,
}

impl AiSettings {
    /// Context level actually allowed: `Off` when disabled.
    pub fn effective_context(&self) -> AiContext {
        if self.enabled {
            self.context
        } else {
            AiContext::Off
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_and_defaults() {
        let s: AiSettings = toml::from_str("enabled = true\ncontext = \"schema+rows\"").unwrap();
        assert_eq!(s.effective_context(), AiContext::SchemaRows);
        let s: AiSettings = toml::from_str("enabled = true").unwrap();
        assert_eq!(s.context, AiContext::Schema);
        assert_eq!(AiSettings::default().effective_context(), AiContext::Off);
        assert!(toml::from_str::<AiSettings>("context = \"everything\"").is_err());
    }
}
