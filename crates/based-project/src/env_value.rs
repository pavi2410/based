use serde::{Deserialize, Deserializer, Serialize, Serializer};

/// A string value or `{ env = "VAR_NAME" }` indirection.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum EnvOrString {
    Literal(String),
    FromEnv { var: String },
}

impl EnvOrString {
    pub fn resolve(&self) -> String {
        match self {
            Self::Literal(s) => s.clone(),
            Self::FromEnv { var } => std::env::var(var).unwrap_or_default(),
        }
    }
}

impl<'de> Deserialize<'de> for EnvOrString {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        #[derive(Deserialize)]
        #[serde(untagged)]
        enum Raw {
            Literal(String),
            FromEnv { env: String },
        }
        match Raw::deserialize(deserializer)? {
            Raw::Literal(s) => Ok(Self::Literal(s)),
            Raw::FromEnv { env } => Ok(Self::FromEnv { var: env }),
        }
    }
}

impl Serialize for EnvOrString {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        match self {
            Self::Literal(s) => serializer.serialize_str(s),
            Self::FromEnv { var } => {
                #[derive(Serialize)]
                struct EnvWrap<'a> {
                    env: &'a str,
                }
                EnvWrap { env: var }.serialize(serializer)
            }
        }
    }
}
