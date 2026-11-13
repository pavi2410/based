//! How much of a connection an AI assistant may see.

use serde::{Deserialize, Serialize};

/// Per-connection AI access. `Off` unless the connection's `[ai]` table enables it.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum ContextLevel {
    #[default]
    Off,
    #[serde(rename = "schema")]
    Schema,
    #[serde(rename = "schema+rows")]
    SchemaRows,
}

impl ContextLevel {
    /// Whether the model may see row data (samples, query results).
    pub fn shares_rows(self) -> bool {
        self == Self::SchemaRows
    }
}
