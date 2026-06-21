//! Cached schema objects for LSP completion and ER diagram data.

use based_core::EngineKind;

/// A schema object (table, view, collection, etc.) visible to the editor.
#[derive(Debug, Clone)]
pub struct SchemaObject {
    /// Fully-qualified name (e.g. `public.users`).
    pub full_name: String,
    /// Short display label (e.g. `users`).
    pub label: String,
    pub kind: ObjectKind,
    pub columns: Vec<ColumnInfo>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ObjectKind {
    Table,
    View,
    MaterializedView,
    Collection,
    Function,
    Procedure,
}

#[derive(Debug, Clone)]
pub struct ColumnInfo {
    pub name: String,
    pub data_type: String,
    pub nullable: bool,
    pub is_primary_key: bool,
}

/// Lazily-populated cache of schema objects for a single connection.
///
/// Panels request a refresh via [`SchemaCache::mark_stale`]; a background task
/// fills `objects` and notifies subscribers. LSP and autocomplete consume this
/// cache without blocking the UI.
#[derive(Debug, Default, Clone)]
pub struct SchemaCache {
    pub engine: Option<EngineKind>,
    pub objects: Vec<SchemaObject>,
    pub last_refreshed_at: Option<std::time::Instant>,
}

impl SchemaCache {
    pub fn new(engine: EngineKind) -> Self {
        Self {
            engine: Some(engine),
            objects: vec![],
            last_refreshed_at: None,
        }
    }

    /// Returns `true` if the cache has never been populated or is older than 5 minutes.
    pub fn is_stale(&self) -> bool {
        self.last_refreshed_at
            .map(|t| t.elapsed().as_secs() > 300)
            .unwrap_or(true)
    }

    /// Returns objects whose label starts with `prefix` (case-insensitive).
    pub fn complete(&self, prefix: &str) -> Vec<&SchemaObject> {
        let lower = prefix.to_lowercase();
        self.objects
            .iter()
            .filter(|o| o.label.to_lowercase().starts_with(&lower))
            .collect()
    }

    pub fn find_by_name(&self, name: &str) -> Option<&SchemaObject> {
        let lower = name.to_lowercase();
        self.objects
            .iter()
            .find(|o| o.full_name == name || o.label == name || o.label.to_lowercase() == lower)
    }

    /// Mark the cache as needing a refresh on the next load.
    pub fn mark_stale(&mut self) {
        self.last_refreshed_at = None;
    }

    /// Column names on `table_name` matching `prefix` (case-insensitive).
    pub fn complete_columns(&self, table_name: &str, prefix: &str) -> Vec<&ColumnInfo> {
        let lower = prefix.to_lowercase();
        self.find_by_name(table_name)
            .map(|obj| {
                obj.columns
                    .iter()
                    .filter(|c| c.name.to_lowercase().starts_with(&lower))
                    .collect()
            })
            .unwrap_or_default()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample_cache() -> SchemaCache {
        let mut cache = SchemaCache::new(EngineKind::SQLite);
        cache.objects.push(SchemaObject {
            full_name: "docs".into(),
            label: "docs".into(),
            kind: ObjectKind::Table,
            columns: vec![
                ColumnInfo {
                    name: "id".into(),
                    data_type: "INTEGER".into(),
                    nullable: false,
                    is_primary_key: true,
                },
                ColumnInfo {
                    name: "title".into(),
                    data_type: "TEXT".into(),
                    nullable: true,
                    is_primary_key: false,
                },
            ],
        });
        cache
    }

    #[test]
    fn complete_columns_filters_prefix() {
        let cache = sample_cache();
        let cols = cache.complete_columns("docs", "ti");
        assert_eq!(cols.len(), 1);
        assert_eq!(cols[0].name, "title");
    }

    #[test]
    fn find_by_name_is_case_insensitive() {
        let cache = sample_cache();
        assert!(cache.find_by_name("DOCS").is_some());
    }
}
