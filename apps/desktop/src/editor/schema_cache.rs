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
#[derive(Debug, Default)]
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
        self.objects
            .iter()
            .find(|o| o.full_name == name || o.label == name)
    }
}
