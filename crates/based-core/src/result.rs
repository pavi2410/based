//! Tabular results shared by every engine, and the outcome of one SQL statement.

use std::error::Error;
use std::fmt;

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

/// A statement that failed, and where in its text the server says it went wrong.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct QueryError {
    pub message: String,
    /// Byte offset into the statement text, when the server reports one.
    pub position: Option<usize>,
}

impl QueryError {
    /// An error with no position (timeouts, drivers that don't report one).
    pub fn new(message: impl Into<String>) -> Self {
        Self {
            message: message.into(),
            position: None,
        }
    }
}

impl fmt::Display for QueryError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.message)
    }
}

impl Error for QueryError {}
