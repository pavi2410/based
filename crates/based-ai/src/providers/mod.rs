pub mod anthropic;
pub mod ollama;
pub mod openai;

use std::sync::Arc;

pub use anthropic::Anthropic;
pub use ollama::Ollama;
pub use openai::OpenAi;

use crate::error::{AiError, AiErrorKind};
use crate::provider::Provider;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum ProviderKind {
    #[default]
    Anthropic,
    OpenAi,
    Ollama,
}

impl ProviderKind {
    pub const ALL: [Self; 3] = [Self::Anthropic, Self::OpenAi, Self::Ollama];

    /// Stable id, also the keychain namespace and settings value.
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Anthropic => "anthropic",
            Self::OpenAi => "openai",
            Self::Ollama => "ollama",
        }
    }

    pub fn from_id(id: &str) -> Option<Self> {
        Self::ALL.into_iter().find(|k| k.as_str() == id)
    }

    pub fn label(self) -> &'static str {
        match self {
            Self::Anthropic => "Anthropic",
            Self::OpenAi => "OpenAI",
            Self::Ollama => "Ollama (local)",
        }
    }

    pub fn needs_key(self) -> bool {
        self != Self::Ollama
    }

    pub fn default_model(self) -> &'static str {
        match self {
            Self::Anthropic => "claude-sonnet-4-5",
            Self::OpenAi => "gpt-4.1",
            Self::Ollama => "llama3.1",
        }
    }

    /// Builds a provider. `base_url` overrides the default endpoint.
    pub fn build(
        self,
        api_key: Option<&str>,
        base_url: Option<&str>,
    ) -> Result<Arc<dyn Provider>, AiError> {
        let key = if self.needs_key() {
            api_key.filter(|k| !k.is_empty()).ok_or_else(|| {
                AiError::new(
                    AiErrorKind::Auth,
                    format!("{} API key is not set", self.label()),
                )
            })?
        } else {
            ""
        };
        Ok(match self {
            Self::Anthropic => {
                let p = Anthropic::new(key);
                Arc::new(match base_url {
                    Some(u) => p.with_base_url(u),
                    None => p,
                })
            }
            Self::OpenAi => {
                let p = OpenAi::new(key);
                Arc::new(match base_url {
                    Some(u) => p.with_base_url(u),
                    None => p,
                })
            }
            Self::Ollama => {
                let p = Ollama::default();
                Arc::new(match base_url {
                    Some(u) => p.with_base_url(u),
                    None => p,
                })
            }
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ids_round_trip() {
        for k in ProviderKind::ALL {
            assert_eq!(ProviderKind::from_id(k.as_str()), Some(k));
        }
        assert_eq!(ProviderKind::from_id("nope"), None);
    }

    #[test]
    fn keyed_providers_require_a_key() {
        let err = |k: ProviderKind, key| k.build(key, None).err().map(|e| e.kind);
        assert_eq!(err(ProviderKind::Anthropic, None), Some(AiErrorKind::Auth));
        assert_eq!(err(ProviderKind::OpenAi, Some("")), Some(AiErrorKind::Auth));
        assert_eq!(err(ProviderKind::Anthropic, Some("k")), None);
        assert_eq!(err(ProviderKind::Ollama, None), None);
    }

    #[test]
    fn built_provider_reports_name() {
        let p = ProviderKind::OpenAi
            .build(Some("k"), Some("http://localhost:1234/v1"))
            .unwrap();
        assert_eq!(p.name(), "openai");
    }
}
