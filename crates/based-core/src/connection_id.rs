use std::fmt::{Display, Formatter, Result};

use serde::{Deserialize, Serialize};

/// Stable opaque identifier for a connection profile or config key.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(transparent)]
pub struct ConnectionId(pub String);

impl ConnectionId {
    pub fn from_key(key: &str) -> Self {
        Self(key.to_string())
    }
}

impl Display for ConnectionId {
    fn fmt(&self, f: &mut Formatter<'_>) -> Result {
        f.write_str(&self.0)
    }
}
