use std::collections::HashMap;
use std::env;
use std::error::Error;
use std::fmt;

use serde::{Deserialize, Deserializer, Serialize, Serializer};

/// Why a `{ env = "VAR" }` value could not be read.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum EnvVarError {
    /// Set neither in the process environment nor in the based-dir `.env`.
    Missing { var: String },
    /// Set in the process environment, but not valid Unicode.
    NotUnicode { var: String },
}

impl EnvVarError {
    /// The variable a connection needs, when it is simply not set.
    pub fn missing_var(&self) -> Option<&str> {
        match self {
            Self::Missing { var } => Some(var),
            Self::NotUnicode { .. } => None,
        }
    }
}

impl fmt::Display for EnvVarError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Missing { var } => write!(f, "environment variable `{var}` is not set"),
            Self::NotUnicode { var } => {
                write!(f, "environment variable `{var}` is not valid Unicode")
            }
        }
    }
}

impl Error for EnvVarError {}

/// Every use holds a secret (password, URL with credentials, passphrase), so
/// `Debug` hides a literal and shows only an env var's name.
#[derive(Clone, PartialEq, Eq)]
pub enum EnvOrString {
    Literal(String),
    FromEnv { var: String },
}

impl fmt::Debug for EnvOrString {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Literal(s) if s.is_empty() => f.write_str("Literal(<empty>)"),
            Self::Literal(_) => f.write_str("Literal(***)"),
            Self::FromEnv { var } => f.debug_struct("FromEnv").field("var", var).finish(),
        }
    }
}

impl EnvOrString {
    pub fn resolve(&self) -> Result<String, EnvVarError> {
        self.resolve_with(&HashMap::new())
    }

    /// Resolve a literal, then `std::env`, then values from a based-dir `.env` map.
    pub fn resolve_with(&self, file_vars: &HashMap<String, String>) -> Result<String, EnvVarError> {
        match self {
            Self::Literal(s) => Ok(s.clone()),
            Self::FromEnv { var } => match env::var(var) {
                Ok(value) => Ok(value),
                Err(env::VarError::NotPresent) => match file_vars.get(var) {
                    Some(value) => Ok(value.clone()),
                    None => Err(EnvVarError::Missing { var: var.clone() }),
                },
                Err(env::VarError::NotUnicode(_)) => {
                    Err(EnvVarError::NotUnicode { var: var.clone() })
                }
            },
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

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashMap;

    #[test]
    fn debug_hides_a_literal_but_names_the_env_var() {
        let lit = EnvOrString::Literal("hunter2".into());
        assert_eq!(format!("{lit:?}"), "Literal(***)");
        let var = EnvOrString::FromEnv {
            var: "PG_PASS".into(),
        };
        assert_eq!(format!("{var:?}"), r#"FromEnv { var: "PG_PASS" }"#);
    }

    #[test]
    fn literal_resolves_as_is() {
        let value = EnvOrString::Literal("secret".into());
        assert_eq!(value.resolve().unwrap(), "secret");
    }

    #[test]
    fn empty_literal_is_allowed() {
        let value = EnvOrString::Literal(String::new());
        assert_eq!(value.resolve().unwrap(), "");
    }

    #[test]
    fn missing_env_var_is_an_error() {
        let var = "BASED_TEST_ENV_OR_STRING_MISSING";
        // SAFETY: this process-global env var is unique to this test.
        unsafe {
            env::remove_var(var);
        }
        let value = EnvOrString::FromEnv { var: var.into() };
        let err = value.resolve().unwrap_err();
        let msg = err.to_string();
        assert!(msg.contains(var), "error should name the variable: {msg}");
        assert!(
            msg.contains("not set"),
            "error should say the variable is missing: {msg}"
        );
    }

    #[test]
    fn present_env_var_resolves() {
        let var = "BASED_TEST_ENV_OR_STRING_PRESENT";
        // SAFETY: this process-global env var is unique to this test.
        unsafe {
            env::set_var(var, "from-env");
        }
        let value = EnvOrString::FromEnv { var: var.into() };
        let resolved = value.resolve();
        unsafe {
            env::remove_var(var);
        }
        assert_eq!(resolved.unwrap(), "from-env");
    }

    #[test]
    fn empty_but_set_env_var_resolves_empty() {
        let var = "BASED_TEST_ENV_OR_STRING_EMPTY";
        // SAFETY: this process-global env var is unique to this test.
        unsafe {
            env::set_var(var, "");
        }
        let value = EnvOrString::FromEnv { var: var.into() };
        let resolved = value.resolve();
        unsafe {
            env::remove_var(var);
        }
        assert_eq!(resolved.unwrap(), "");
    }

    #[test]
    fn resolve_with_uses_file_vars_when_process_env_missing() {
        let var = "BASED_TEST_ENV_OR_STRING_FILE_ONLY";
        unsafe {
            env::remove_var(var);
        }
        let mut file_vars = HashMap::new();
        file_vars.insert(var.to_string(), "from-file".into());
        let value = EnvOrString::FromEnv { var: var.into() };
        assert_eq!(value.resolve_with(&file_vars).unwrap(), "from-file");
    }

    #[test]
    fn resolve_with_prefers_process_env_over_file_vars() {
        let var = "BASED_TEST_ENV_OR_STRING_BOTH";
        unsafe {
            env::set_var(var, "from-process");
        }
        let mut file_vars = HashMap::new();
        file_vars.insert(var.to_string(), "from-file".into());
        let value = EnvOrString::FromEnv { var: var.into() };
        let resolved = value.resolve_with(&file_vars);
        unsafe {
            env::remove_var(var);
        }
        assert_eq!(resolved.unwrap(), "from-process");
    }
}
