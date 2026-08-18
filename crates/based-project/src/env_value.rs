use std::collections::HashMap;
use std::env;

use anyhow::{Result, bail};
use serde::{Deserialize, Deserializer, Serialize, Serializer};

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum EnvOrString {
    Literal(String),
    FromEnv { var: String },
}

impl EnvOrString {
    pub fn resolve(&self) -> Result<String> {
        self.resolve_with(&HashMap::new())
    }

    /// Resolve a literal, then `std::env`, then values from a based-dir `.env` map.
    pub fn resolve_with(&self, file_vars: &HashMap<String, String>) -> Result<String> {
        match self {
            Self::Literal(s) => Ok(s.clone()),
            Self::FromEnv { var } => match env::var(var) {
                Ok(value) => Ok(value),
                Err(env::VarError::NotPresent) => match file_vars.get(var) {
                    Some(value) => Ok(value.clone()),
                    None => bail!("environment variable `{var}` is not set"),
                },
                Err(env::VarError::NotUnicode(_)) => {
                    bail!("environment variable `{var}` is not valid Unicode")
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
