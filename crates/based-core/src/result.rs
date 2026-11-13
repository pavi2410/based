//! Tabular results shared by every engine, and the outcome of one SQL statement.

use crate::Value;

/// One result column. `type_name` is the server's type name when the engine has
/// one (Postgres, SQLite); document and key-value engines leave it `None`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ResultColumn {
    pub name: String,
    pub type_name: Option<String>,
}

impl ResultColumn {
    pub fn new(name: impl Into<String>, type_name: Option<String>) -> Self {
        Self {
            name: name.into(),
            type_name,
        }
    }
}

/// A grid of values: what the results grid, export and AI tools consume.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct ResultSet {
    pub columns: Vec<ResultColumn>,
    pub rows: Vec<Vec<Value>>,
}

impl ResultSet {
    pub fn column_names(&self) -> impl Iterator<Item = &str> {
        self.columns.iter().map(|c| c.name.as_str())
    }
}

/// What one SQL statement produced.
#[derive(Debug, Clone, PartialEq)]
pub enum SqlOutcome {
    /// A statement that returns rows (SELECT, VALUES, RETURNING, PRAGMA, …).
    Rows(ResultSet),
    /// A statement that changes data or schema, with the server's affected count.
    Affected(u64),
}
