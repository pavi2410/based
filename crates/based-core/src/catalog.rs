//! Database objects as the engines list them: tables, views, collections, keys.

use serde::{Deserialize, Serialize};

use crate::ObjectRef;

/// What kind of object a catalog entry is. Serialized in saved sessions
/// (`ObjectInfo` tabs): keep the names stable.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum ObjectKind {
    Table,
    View,
    #[serde(rename = "matview")]
    MaterializedView,
    Trigger,
    Collection,
    Key,
}

impl ObjectKind {
    /// Short lowercase name, e.g. for tab titles.
    pub fn label(self) -> &'static str {
        match self {
            Self::Table => "table",
            Self::View => "view",
            Self::MaterializedView => "matview",
            Self::Trigger => "trigger",
            Self::Collection => "collection",
            Self::Key => "key",
        }
    }

    /// Heading the object is listed under.
    pub fn group(self) -> &'static str {
        match self {
            Self::Table => "Tables",
            Self::View | Self::MaterializedView => "Views",
            Self::Trigger => "Triggers",
            Self::Collection => "Collections",
            Self::Key => "Keys",
        }
    }
}

/// One listed object. Postgres sets `schema`; the other engines leave it `None`.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct DbObject {
    pub name: String,
    pub schema: Option<String>,
    pub kind: ObjectKind,
}

impl DbObject {
    pub fn new(schema: Option<String>, name: impl Into<String>, kind: ObjectKind) -> Self {
        Self {
            name: name.into(),
            schema,
            kind,
        }
    }

    /// The object as tabs refer to it.
    pub fn object_ref(&self) -> ObjectRef {
        ObjectRef {
            schema: self.schema.clone(),
            name: self.name.clone(),
        }
    }

    /// `schema.name`, or the bare name without a schema.
    pub fn display_name(&self) -> String {
        match &self.schema {
            Some(schema) => format!("{schema}.{}", self.name),
            None => self.name.clone(),
        }
    }
}

/// One column of a table or view, with the server's type name.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct DbColumn {
    pub name: String,
    pub data_type: String,
}

/// A table or view and its columns, in column order.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Relation {
    pub object: DbObject,
    pub columns: Vec<DbColumn>,
}
