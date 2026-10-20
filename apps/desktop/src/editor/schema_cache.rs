//! Cached schema objects for LSP completion and ER diagram data.

use based_core::EngineKind;
use std::time::Instant;

/// A schema object (table, view, collection, etc.) visible to the editor.
#[derive(Debug, Clone)]
pub struct SchemaObject {
    /// Fully-qualified name (e.g. `public.users`).
    pub full_name: String,
    /// Short display label (e.g. `users`).
    pub label: String,
    /// Owning schema, when the engine has them (`None` for SQLite).
    pub schema: Option<String>,
    pub kind: ObjectKind,
    pub columns: Vec<ColumnInfo>,
}

impl SchemaObject {
    /// Text shown in the menu: `schema.table` outside `public`, else the bare name.
    pub fn display(&self) -> String {
        match self.schema.as_deref() {
            Some(s) if s != "public" => format!("{s}.{}", self.label),
            _ => self.label.clone(),
        }
    }

    /// SQL to insert: identifiers quoted where needed.
    pub fn insert_text(&self) -> String {
        match self.schema.as_deref() {
            Some(s) if s != "public" => format!("{}.{}", quote_ident(s), quote_ident(&self.label)),
            _ => quote_ident(&self.label),
        }
    }
}

/// Quote `name` with `"` unless it is a plain lowercase identifier.
pub fn quote_ident(name: &str) -> String {
    let plain = name
        .chars()
        .next()
        .is_some_and(|c| c.is_ascii_lowercase() || c == '_')
        && name
            .chars()
            .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '_');
    if plain {
        name.to_string()
    } else {
        format!("\"{}\"", name.replace('"', "\"\""))
    }
}

/// Strip surrounding quotes from one identifier part, undoing `""` escapes.
pub fn unquote(part: &str) -> String {
    let p = part.trim();
    match p.strip_prefix('"') {
        Some(rest) => rest.strip_suffix('"').unwrap_or(rest).replace("\"\"", "\""),
        None => p.to_string(),
    }
}

/// Split `a."b.c".d` on dots outside quotes, unquoting each part.
pub fn split_dotted(s: &str) -> Vec<String> {
    let (mut parts, mut start, mut inq) = (Vec::new(), 0, false);
    for (i, c) in s.char_indices() {
        match c {
            '"' => inq = !inq,
            '.' if !inq => {
                parts.push(unquote(&s[start..i]));
                start = i + 1;
            }
            _ => {}
        }
    }
    parts.push(unquote(&s[start..]));
    parts
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ObjectKind {
    Table,
    View,
}

#[derive(Debug, Clone)]
pub struct ColumnInfo {
    pub name: String,
    pub data_type: String,
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
    pub last_refreshed_at: Option<Instant>,
}

impl SchemaCache {
    pub fn new(engine: EngineKind) -> Self {
        Self {
            engine: Some(engine),
            objects: vec![],
            last_refreshed_at: None,
        }
    }

    /// Returns objects whose label starts with `prefix` (case-insensitive).
    pub fn complete(&self, prefix: &str) -> Vec<&SchemaObject> {
        let lower = unquote(prefix).to_lowercase();
        self.objects
            .iter()
            .filter(|o| {
                o.label.to_lowercase().starts_with(&lower)
                    || o.display().to_lowercase().starts_with(&lower)
            })
            .collect()
    }

    pub fn find_by_name(&self, name: &str) -> Option<&SchemaObject> {
        let parts = split_dotted(name);
        let (table, schema) = match parts.as_slice() {
            [t] => (t.as_str(), None),
            [.., s, t] => (t.as_str(), Some(s.as_str())),
            [] => return None,
        };
        let lower = table.to_lowercase();
        let matches = |exact: bool| {
            self.objects.iter().find(|o| {
                let same = if exact {
                    o.label == table
                } else {
                    o.label.to_lowercase() == lower
                };
                same && schema.is_none_or(|s| o.schema.as_deref() == Some(s))
            })
        };
        matches(true).or_else(|| matches(false))
    }

    /// Column names on `table_name` matching `prefix` (case-insensitive).
    pub fn complete_columns(&self, table_name: &str, prefix: &str) -> Vec<&ColumnInfo> {
        let lower = unquote(prefix).to_lowercase();
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
            schema: None,
            kind: ObjectKind::Table,
            columns: vec![
                ColumnInfo {
                    name: "id".into(),
                    data_type: "INTEGER".into(),
                },
                ColumnInfo {
                    name: "title".into(),
                    data_type: "TEXT".into(),
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
    fn quotes_only_when_needed() {
        assert_eq!(quote_ident("users"), "users");
        assert_eq!(quote_ident("my table"), "\"my table\"");
        assert_eq!(quote_ident("Users"), "\"Users\"");
        assert_eq!(quote_ident("a\"b"), "\"a\"\"b\"");
    }

    #[test]
    fn splits_quoted_dotted_names() {
        assert_eq!(split_dotted("\"a.b\".c"), vec!["a.b", "c"]);
        assert!(sample_cache().find_by_name("\"docs\"").is_some());
    }

    #[test]
    fn find_by_name_is_case_insensitive() {
        let cache = sample_cache();
        assert!(cache.find_by_name("DOCS").is_some());
    }
}
