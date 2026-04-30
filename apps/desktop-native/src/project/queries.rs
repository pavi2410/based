// Saved queries — .based/queries/*.query.toml read/write.
// Implemented in Phase 2.

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SavedQuery {
    pub name: String,
    pub sql: String,
    /// Optional hint — the key of the preferred connection in config.toml.
    pub default_connection: Option<String>,
}
