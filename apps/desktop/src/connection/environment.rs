//! Deployment environment of a connection, read from reserved tags.
//!
//! No file-format change: `prod`, `staging` and `dev` tags (and their long
//! forms) mark the environment, so existing project files keep working.

use gpui_kit::Hsla;
use gpui_kit::component::Theme;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Environment {
    Production,
    Staging,
    Development,
}

impl Environment {
    pub const ALL: [Self; 3] = [Self::Development, Self::Staging, Self::Production];

    /// First reserved tag wins; `None` when the connection is unmarked.
    pub fn from_tags(tags: &[String]) -> Option<Self> {
        tags.iter().find_map(|t| Self::from_tag(t))
    }

    pub fn from_tag(tag: &str) -> Option<Self> {
        match tag.trim().to_ascii_lowercase().as_str() {
            "prod" | "production" => Some(Self::Production),
            "stg" | "staging" => Some(Self::Staging),
            "dev" | "development" => Some(Self::Development),
            _ => None,
        }
    }

    /// Short tag written back to the project file.
    pub fn tag(self) -> &'static str {
        match self {
            Self::Production => "prod",
            Self::Staging => "staging",
            Self::Development => "dev",
        }
    }

    pub fn label(self) -> &'static str {
        match self {
            Self::Production => "Production",
            Self::Staging => "Staging",
            Self::Development => "Development",
        }
    }

    pub fn color(self, t: &Theme) -> Hsla {
        match self {
            Self::Production => t.danger,
            Self::Staging => t.warning,
            Self::Development => t.green,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reads_reserved_tags_case_insensitively() {
        assert_eq!(
            Environment::from_tags(&["team".into(), "PROD".into()]),
            Some(Environment::Production)
        );
        assert_eq!(
            Environment::from_tags(&["Staging".into()]),
            Some(Environment::Staging)
        );
        assert_eq!(Environment::from_tags(&["local".into()]), None);
    }

    #[test]
    fn tag_round_trips() {
        for env in Environment::ALL {
            assert_eq!(Environment::from_tag(env.tag()), Some(env));
        }
    }
}
