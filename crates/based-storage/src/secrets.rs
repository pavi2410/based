//! OS keychain boundary for connection-template secrets.
//!
//! Non-secret metadata lives in SQLite; literal passwords and other secrets are
//! stored under `based/<ref>` accounts in the platform credential store.

use anyhow::{Context, Result};
use keyring::Entry;

const SERVICE: &str = "based";

#[derive(Debug, Clone)]
pub struct SecretStore;

impl Default for SecretStore {
    fn default() -> Self {
        Self
    }
}

impl SecretStore {
    pub fn new() -> Self {
        Self
    }

    fn entry(ref_key: &str) -> Result<Entry> {
        Entry::new(SERVICE, ref_key).context("open keychain entry")
    }

    pub fn get(&self, ref_key: &str) -> Result<Option<String>> {
        match Self::entry(ref_key)?.get_password() {
            Ok(value) => Ok(Some(value)),
            Err(keyring::Error::NoEntry) => Ok(None),
            Err(err) => Err(err).context("read keychain secret"),
        }
    }

    pub fn set(&self, ref_key: &str, value: &str) -> Result<()> {
        Self::entry(ref_key)?
            .set_password(value)
            .context("write keychain secret")
    }

    pub fn delete(&self, ref_key: &str) -> Result<()> {
        match Self::entry(ref_key)?.delete_credential() {
            Ok(()) | Err(keyring::Error::NoEntry) => Ok(()),
            Err(err) => Err(err).context("delete keychain secret"),
        }
    }

    /// Stable ref for a connection-template password field.
    pub fn template_password_ref(template_id: uuid::Uuid) -> String {
        format!("connection-template:{template_id}:password")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn template_password_ref_is_stable() {
        let id = uuid::Uuid::nil();
        assert_eq!(
            SecretStore::template_password_ref(id),
            "connection-template:00000000-0000-0000-0000-000000000000:password"
        );
    }
}
