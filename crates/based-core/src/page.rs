//! Engine-agnostic table page, as returned by headless engine crates.

use std::fmt;

use crate::{ObjectRef, Value};

/// One page of a table browse. `columns` is `None` when the page is empty.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct RawPage {
    pub total: i64,
    pub columns: Option<Vec<String>>,
    pub rows: Vec<Vec<Value>>,
}

/// Per-column metadata for table browse headers.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct ColumnMeta {
    pub data_type: Option<String>,
    pub nullable: Option<bool>,
    pub is_primary_key: bool,
    pub is_unique: bool,
    /// The column this one references, when it is (part of) a foreign key.
    pub fk: Option<FkTarget>,
}

impl ColumnMeta {
    pub fn is_foreign_key(&self) -> bool {
        self.fk.is_some()
    }
}

/// A foreign key's referenced table and the column this one maps to. For a
/// composite key, each local column carries its own counterpart.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct FkTarget {
    pub table: ObjectRef,
    pub column: String,
}

impl fmt::Display for FkTarget {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}({})", self.table, self.column)
    }
}

/// Headed string table (inspector sections).
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Grid {
    pub headers: Vec<&'static str>,
    pub rows: Vec<Vec<String>>,
}

impl Grid {
    pub fn new(headers: &[&'static str], rows: Vec<Vec<String>>) -> Self {
        Self {
            headers: headers.to_vec(),
            rows,
        }
    }
}

/// Engine-specific extra inspector section.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TableExtra {
    Ddl(String),
    Stats(Vec<(String, String)>),
}

/// Table structure as shown by the inspector.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TableInfo {
    pub columns: Grid,
    pub indexes: Grid,
    pub constraints: Grid,
    pub extra: TableExtra,
}

/// Quote a SQL identifier (`a"b` → `"a""b"`).
pub fn quote_ident(ident: &str) -> String {
    format!("\"{}\"", ident.replace('"', "\"\""))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn quotes_embedded_quotes() {
        assert_eq!(quote_ident(r#"a"b"#), r#""a""b""#);
    }
}
