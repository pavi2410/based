//! AI assistant preferences. API keys live in the OS keychain, never here.

use super::*;
use based_ai::ProviderKind;

#[derive(Clone, Debug, Default, Serialize, Deserialize, PartialEq, Eq)]
pub struct AiPreferences {
    /// `ProviderKind` id; empty or unknown falls back to the default provider.
    #[serde(default)]
    provider: String,
    /// Empty means the provider's default model.
    #[serde(default)]
    model: String,
    /// Empty means the provider's default endpoint.
    #[serde(default)]
    base_url: String,
    /// Suggest completions as ghost text while typing. Off by default: it sends the text
    /// around the cursor to the provider on every pause.
    #[serde(default)]
    inline: bool,
}

impl AiPreferences {
    pub fn provider(&self) -> ProviderKind {
        ProviderKind::from_id(&self.provider).unwrap_or_default()
    }

    pub fn model(&self) -> &str {
        let model = self.model.trim();
        if model.is_empty() {
            self.provider().default_model()
        } else {
            model
        }
    }

    /// Switching provider drops the model, which belongs to the old provider.
    fn set_provider(&mut self, kind: ProviderKind) -> bool {
        if self.provider() == kind && self.provider == kind.as_str() {
            return false;
        }
        self.provider = kind.as_str().to_owned();
        self.model.clear();
        true
    }

    pub fn inline(&self) -> bool {
        self.inline
    }

    pub fn base_url(&self) -> Option<&str> {
        Some(self.base_url.trim()).filter(|u| !u.is_empty())
    }
}

pub fn ai_prefs(cx: &App) -> AiPreferences {
    cx.global::<NativePreferences>().ai.clone()
}

fn update_ai(update: impl FnOnce(&mut AiPreferences) -> bool, cx: &mut App) {
    let changed = cx.update_global(|p: &mut NativePreferences, _| {
        let changed = update(&mut p.ai);
        if changed {
            p.save_best_effort();
        }
        changed
    });
    if changed {
        cx.refresh_windows();
    }
}

pub fn set_ai_provider(kind: ProviderKind, cx: &mut App) {
    update_ai(|p| p.set_provider(kind), cx);
}

pub fn set_ai_model(model: String, cx: &mut App) {
    update_ai(
        |p| {
            if p.model == model {
                return false;
            }
            p.model = model;
            true
        },
        cx,
    );
}

pub fn set_ai_inline(on: bool, cx: &mut App) {
    update_ai(
        |p| {
            let changed = p.inline != on;
            p.inline = on;
            changed
        },
        cx,
    );
}

pub fn set_ai_base_url(url: String, cx: &mut App) {
    update_ai(
        |p| {
            if p.base_url == url {
                return false;
            }
            p.base_url = url;
            true
        },
        cx,
    );
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn defaults_fall_back_to_provider_defaults() {
        let p = AiPreferences::default();
        assert_eq!(p.provider(), ProviderKind::Anthropic);
        assert_eq!(p.model(), ProviderKind::Anthropic.default_model());
        assert_eq!(p.base_url(), None);
    }

    #[test]
    fn unknown_provider_and_blank_values_are_ignored() {
        let p = AiPreferences {
            provider: "nope".into(),
            model: "  ".into(),
            base_url: " ".into(),
            ..AiPreferences::default()
        };
        assert_eq!(p.provider(), ProviderKind::Anthropic);
        assert_eq!(p.model(), ProviderKind::Anthropic.default_model());
        assert_eq!(p.base_url(), None);
    }

    #[test]
    fn custom_values_win() {
        let p = AiPreferences {
            provider: "ollama".into(),
            model: " qwen2.5 ".into(),
            base_url: "http://box:11434".into(),
            ..AiPreferences::default()
        };
        assert_eq!(p.provider(), ProviderKind::Ollama);
        assert_eq!(p.model(), "qwen2.5");
        assert_eq!(p.base_url(), Some("http://box:11434"));
    }

    #[test]
    fn switching_provider_resets_model() {
        let mut p = AiPreferences {
            provider: "openai".into(),
            model: "gpt-x".into(),
            base_url: String::new(),
            ..AiPreferences::default()
        };
        assert!(!p.set_provider(ProviderKind::OpenAi));
        assert_eq!(p.model(), "gpt-x");
        assert!(p.set_provider(ProviderKind::Ollama));
        assert_eq!(p.model(), ProviderKind::Ollama.default_model());
    }

    #[test]
    fn old_prefs_files_without_ai_table_still_load() {
        let prefs: NativePreferences = toml::from_str("page_size = 100").unwrap();
        assert_eq!(prefs.ai, AiPreferences::default());
    }
}
