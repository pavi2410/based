//! Engine-agnostic table page, as returned by headless engine crates.

use std::fmt;
use std::ops::Index;

use crate::{ObjectRef, ResultSet};

/// One page of a table browse. An empty page has no result columns; callers
/// fall back to the table's catalog order.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct RawPage {
    pub total: i64,
    pub set: ResultSet,
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

/// Column metadata for one table, in the table's column order.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct ColumnCatalog(Vec<(String, ColumnMeta)>);

impl ColumnCatalog {
    /// Append a column (callers add them in ordinal order).
    pub fn push(&mut self, name: impl Into<String>, meta: ColumnMeta) {
        self.0.push((name.into(), meta));
    }

    pub fn get(&self, name: &str) -> Option<&ColumnMeta> {
        self.0.iter().find(|(n, _)| n == name).map(|(_, m)| m)
    }

    pub fn get_mut(&mut self, name: &str) -> Option<&mut ColumnMeta> {
        self.0.iter_mut().find(|(n, _)| n == name).map(|(_, m)| m)
    }

    /// Column names in table order.
    pub fn names(&self) -> impl Iterator<Item = &str> {
        self.0.iter().map(|(n, _)| n.as_str())
    }

    pub fn is_empty(&self) -> bool {
        self.0.is_empty()
    }

    /// Primary-key columns in table order; empty for keyless tables and views.
    pub fn primary_key(&self) -> impl Iterator<Item = &str> {
        self.0
            .iter()
            .filter(|(_, m)| m.is_primary_key)
            .map(|(n, _)| n.as_str())
    }
}

/// The column a table page is sorted by, chosen in the grid.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PageSort {
    pub column: String,
    pub ascending: bool,
}

/// ` ORDER BY …` for a table page, so the database sorts the whole table and pages
/// stay stable: the chosen column, then the primary key to break ties; the primary
/// key alone by default. Empty when there is neither (views, keyless tables), where
/// the server's order is all there is.
pub fn page_order_sql(catalog: &ColumnCatalog, sort: Option<&PageSort>) -> String {
    let mut terms: Vec<String> = Vec::new();
    if let Some(sort) = sort {
        let dir = if sort.ascending { "ASC" } else { "DESC" };
        terms.push(format!("{} {dir}", quote_ident(&sort.column)));
    }
    terms.extend(
        catalog
            .primary_key()
            .filter(|pk| sort.is_none_or(|s| s.column != *pk))
            .map(quote_ident),
    );
    if terms.is_empty() {
        String::new()
    } else {
        format!(" ORDER BY {}", terms.join(", "))
    }
}

impl Index<&str> for ColumnCatalog {
    type Output = ColumnMeta;

    /// Panics when the column is missing; use [`ColumnCatalog::get`] otherwise.
    fn index(&self, name: &str) -> &ColumnMeta {
        self.get(name)
            .unwrap_or_else(|| panic!("no column {name:?} in catalog"))
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

/// Quote a SQL identifier (`a"b` → `"a""b"`).
pub fn quote_ident(ident: &str) -> String {
    format!("\"{}\"", ident.replace('"', "\"\""))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn catalog(cols: &[(&str, bool)]) -> ColumnCatalog {
        let mut c = ColumnCatalog::default();
        for (name, pk) in cols {
            c.push(
                *name,
                ColumnMeta {
                    is_primary_key: *pk,
                    ..Default::default()
                },
            );
        }
        c
    }

    #[test]
    fn pages_follow_the_primary_key_by_default() {
        let c = catalog(&[("a", true), ("b", true), ("name", false)]);
        assert_eq!(page_order_sql(&c, None), r#" ORDER BY "a", "b""#);
    }

    #[test]
    fn a_chosen_sort_breaks_ties_on_the_key() {
        let c = catalog(&[("id", true), ("total", false)]);
        let sort = PageSort {
            column: "total".into(),
            ascending: false,
        };
        assert_eq!(
            page_order_sql(&c, Some(&sort)),
            r#" ORDER BY "total" DESC, "id""#
        );
        let by_id = PageSort {
            column: "id".into(),
            ascending: true,
        };
        assert_eq!(page_order_sql(&c, Some(&by_id)), r#" ORDER BY "id" ASC"#);
    }

    #[test]
    fn keyless_tables_keep_server_order() {
        assert_eq!(page_order_sql(&catalog(&[("x", false)]), None), "");
    }

    #[test]
    fn quotes_embedded_quotes() {
        assert_eq!(quote_ident(r#"a"b"#), r#""a""b""#);
    }
}
