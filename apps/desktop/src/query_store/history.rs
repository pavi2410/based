//! Re-export query history from `based-query`.

pub use based_query::{
    HistoryEntry, QueryHistory, RunStatus, MAX_HISTORY_PER_CONNECTION,
};

/// Legacy alias used in a few call sites.
pub const MAX_HISTORY: usize = MAX_HISTORY_PER_CONNECTION;
