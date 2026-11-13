//! A table's structure as the structure tab shows it: one model for every SQL
//! engine, filled from the engine's own catalog.

use crate::ObjectRef;

/// Everything the structure tab shows about one table.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct TableSpec {
    /// In table order.
    pub columns: Vec<ColumnSpec>,
    pub indexes: Vec<IndexSpec>,
    pub constraints: Vec<ConstraintSpec>,
    /// Foreign keys in other tables that point at this one.
    pub referenced_by: Vec<Reference>,
    /// The planner's estimate (Postgres) or `sqlite_stat1`; `None` when unknown.
    pub row_estimate: Option<i64>,
    pub sizes: Option<TableSizes>,
    /// `CREATE TABLE …` plus its indexes; empty when the engine can't give one.
    pub ddl: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ColumnSpec {
    pub name: String,
    /// As the engine names it: `text[]`, `numeric(10,2)`, a SQLite declared type.
    pub data_type: String,
    pub nullable: bool,
    /// The default expression, verbatim.
    pub default: Option<String>,
    pub primary_key: bool,
    /// The column this one references, for a single-column foreign key.
    pub references: Option<ColumnRef>,
}

/// A column in another table.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ColumnRef {
    pub table: ObjectRef,
    pub column: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct IndexSpec {
    pub name: String,
    pub unique: bool,
    /// `btree`, `gin`, …; `None` where the engine has one kind.
    pub method: Option<String>,
    /// Indexed columns in key order; expressions show as written.
    pub columns: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ConstraintSpec {
    pub name: String,
    /// `PRIMARY KEY`, `FOREIGN KEY`, `UNIQUE`, `CHECK`, …
    pub kind: String,
    pub definition: String,
}

/// A foreign key elsewhere that points here.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Reference {
    pub from_table: ObjectRef,
    pub from_columns: Vec<String>,
    pub to_columns: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TableSizes {
    pub table: String,
    pub indexes: String,
    pub total: String,
}

impl TableSpec {
    /// Indexes that contain `column`, by name.
    pub fn indexes_on(&self, column: &str) -> Vec<&str> {
        self.indexes
            .iter()
            .filter(|i| i.columns.iter().any(|c| c == column))
            .map(|i| i.name.as_str())
            .collect()
    }

    /// A foreign key column no index leads with: joins and lookups on it scan.
    pub fn is_unindexed_foreign_key(&self, column: &ColumnSpec) -> bool {
        column.references.is_some()
            && !self
                .indexes
                .iter()
                .any(|i| i.columns.first().is_some_and(|c| *c == column.name))
    }

    /// The primary key's columns, in table order.
    pub fn primary_key(&self) -> Vec<&str> {
        self.columns
            .iter()
            .filter(|c| c.primary_key)
            .map(|c| c.name.as_str())
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn column(name: &str, fk: bool) -> ColumnSpec {
        ColumnSpec {
            name: name.into(),
            data_type: "integer".into(),
            nullable: false,
            default: None,
            primary_key: name == "id",
            references: fk.then(|| ColumnRef {
                table: ObjectRef::bare("customers"),
                column: "id".into(),
            }),
        }
    }

    fn index(name: &str, columns: &[&str]) -> IndexSpec {
        IndexSpec {
            name: name.into(),
            unique: false,
            method: None,
            columns: columns.iter().map(|c| c.to_string()).collect(),
        }
    }

    #[test]
    fn a_foreign_key_needs_an_index_that_leads_with_it() {
        let spec = TableSpec {
            columns: vec![column("id", false), column("customer_id", true)],
            indexes: vec![
                index("pk", &["id"]),
                index("by_date", &["placed_at", "customer_id"]),
            ],
            ..Default::default()
        };
        let fk = spec.columns[1].clone();
        assert!(
            spec.is_unindexed_foreign_key(&fk),
            "trailing columns don't serve lookups"
        );
        assert_eq!(spec.indexes_on("customer_id"), ["by_date"]);
        assert_eq!(spec.primary_key(), ["id"]);

        let covered = TableSpec {
            indexes: vec![index("by_customer", &["customer_id"])],
            ..spec
        };
        assert!(!covered.is_unindexed_foreign_key(&fk));
    }
}
