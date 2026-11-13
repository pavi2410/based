//! Query persistence, variable substitution, and SQL utilities (no UI).

pub mod history;
pub mod lint;
pub mod resolve;
pub mod saved;
pub mod sql;
pub mod variables;
pub mod write;

pub use history::{HistoryEntry, MAX_HISTORY_PER_CONNECTION, QueryHistory, RunOutcome};
pub use lint::{Lint, Severity, syntax_lints};
pub use resolve::{ResolveError, VariableContext, resolve_query};
pub use saved::{SavedQueries, SavedQuery};
pub use sql::{format_sql, has_ambiguous_quoting, keywords, split_statements, statement_at};
pub use variables::{Variables, load_variables, save_variables, substitute_dollar_vars};
pub use write::{changes_data, write_summary};
