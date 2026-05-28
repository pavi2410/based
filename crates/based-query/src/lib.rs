//! Query persistence, variable substitution, and SQL utilities (no UI).

pub mod history;
pub mod resolve;
pub mod saved;
pub mod sql;
pub mod variables;

pub use history::{HistoryEntry, QueryHistory, RunStatus, MAX_HISTORY_PER_CONNECTION};
pub use resolve::{ResolveError, VariableContext, resolve_query};
pub use saved::{SavedQueries, SavedQuery};
pub use sql::{SqlStatement, statement_at_offset, statements_in_script};
pub use variables::{Variables, load_variables, save_variables, substitute_dollar_vars};
