//! Provider API keys, kept beside connection passwords in the personal `.env`
//! (`~/.config/based/.env`, owner-only). Blocking file I/O: call off the UI thread.

use std::path::PathBuf;

use anyhow::Result;
use based_project::{load_env_file, remove_env_key, upsert_env_file};

use crate::project::personal::personal_root;

fn env_path() -> PathBuf {
    personal_root().join(".env")
}

fn var(provider: &str) -> String {
    format!("BASED_AI_{}_API_KEY", provider.to_ascii_uppercase())
}

pub fn get(provider: &str) -> Result<Option<String>> {
    let mut vars = load_env_file(&env_path())?;
    Ok(vars.remove(&var(provider)).filter(|k| !k.is_empty()))
}

pub fn set(provider: &str, key: &str) -> Result<()> {
    upsert_env_file(&env_path(), &var(provider), key)
}

pub fn delete(provider: &str) -> Result<()> {
    remove_env_key(&env_path(), &var(provider))
}
