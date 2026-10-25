//! Provider API keys, kept beside connection passwords in the personal `.env`
//! (`~/.config/based/.env`, owner-only). Blocking file I/O: call off the UI thread.

use std::path::{Path, PathBuf};

use anyhow::Result;
use based_project::{load_env_file, remove_env_key, upsert_env_file};

fn env_path(personal_root: &Path) -> PathBuf {
    personal_root.join(".env")
}

fn var(provider: &str) -> String {
    format!("BASED_AI_{}_API_KEY", provider.to_ascii_uppercase())
}

pub fn get(personal_root: &Path, provider: &str) -> Result<Option<String>> {
    let mut vars = load_env_file(&env_path(personal_root))?;
    Ok(vars.remove(&var(provider)).filter(|k| !k.is_empty()))
}

pub fn set(personal_root: &Path, provider: &str, key: &str) -> Result<()> {
    upsert_env_file(&env_path(personal_root), &var(provider), key)
}

pub fn delete(personal_root: &Path, provider: &str) -> Result<()> {
    remove_env_key(&env_path(personal_root), &var(provider))
}
