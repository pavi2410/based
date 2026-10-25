use futures::future::BoxFuture;

use crate::error::AiError;

/// How much of a connection the model may see.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum ContextLevel {
    Off,
    #[default]
    Schema,
    SchemaRows,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EntityKind {
    Table,
    View,
    Collection,
    KeyPattern,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Field {
    pub name: String,
    pub ty: Option<String>,
    pub nullable: bool,
    pub primary_key: bool,
    /// `table.column` this field references.
    pub references: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Entity {
    pub name: String,
    pub kind: EntityKind,
    pub fields: Vec<Field>,
    /// A few example rows, one string per field. Only rendered at `SchemaRows`.
    pub samples: Vec<Vec<String>>,
}

impl Entity {
    pub fn new(name: impl Into<String>, kind: EntityKind, fields: Vec<Field>) -> Self {
        Self {
            name: name.into(),
            kind,
            fields,
            samples: Vec::new(),
        }
    }
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct SchemaSnapshot {
    pub entities: Vec<Entity>,
}

/// Implemented per engine (in the app or engine crates) to load a snapshot.
pub trait SchemaSource: Send + Sync {
    fn snapshot(&self) -> BoxFuture<'_, Result<SchemaSnapshot, AiError>>;
}
