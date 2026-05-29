//! Local SQLite metadata store with WAL and keychain secret boundary.

mod migrate;
mod paths;
mod secrets;
mod session_keys;
mod store;

pub use paths::{default_db_path, ensure_parent};
pub use secrets::SecretStore;
pub use session_keys::{
    ACTIVE_CONNECTION_ID, ACTIVE_ENVIRONMENT_ID, ACTIVE_TAB_INDEX, ACTIVE_WORKSPACE_ID, OPEN_TABS,
};
pub use store::{MetadataStore, WorkspaceSummary};
