//! A reference to a database object (table, view, collection, key, …).

use std::fmt;

use serde::{Deserialize, Deserializer, Serialize};

/// An object by schema and name, kept apart so names containing dots stay whole.
/// `schema` is set for engines with schemas (Postgres) and `None` otherwise.
#[derive(Debug, Clone, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize)]
pub struct ObjectRef {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub schema: Option<String>,
    pub name: String,
}

impl ObjectRef {
    /// An object in a schema.
    pub fn qualified(schema: impl Into<String>, name: impl Into<String>) -> Self {
        Self {
            schema: Some(schema.into()),
            name: name.into(),
        }
    }

    /// An object in an engine without schemas (SQLite table, Mongo collection, Redis key).
    pub fn bare(name: impl Into<String>) -> Self {
        Self {
            schema: None,
            name: name.into(),
        }
    }

    /// Read a `schema.name` string written before objects were structured: split
    /// at the last dot, with `default_schema` when there is none. Only for data
    /// that predates `ObjectRef` — a dotted name is ambiguous here.
    pub fn from_legacy_qualified(text: &str, default_schema: &str) -> Self {
        match text.rsplit_once('.') {
            Some((schema, name)) if !name.is_empty() => Self::qualified(schema, name),
            _ => Self::qualified(default_schema, text),
        }
    }
}

impl fmt::Display for ObjectRef {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match &self.schema {
            Some(schema) => write!(f, "{schema}.{}", self.name),
            None => f.write_str(&self.name),
        }
    }
}

/// Accepts the structured form and, from old sessions, a plain string (read as a
/// bare name; callers that know the engine fix up Postgres with
/// [`ObjectRef::from_legacy_qualified`]).
impl<'de> Deserialize<'de> for ObjectRef {
    fn deserialize<D: Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
        #[derive(Deserialize)]
        #[serde(untagged)]
        enum Repr {
            Structured {
                #[serde(default)]
                schema: Option<String>,
                name: String,
            },
            Legacy(String),
        }
        Ok(match Repr::deserialize(d)? {
            Repr::Structured { schema, name } => Self { schema, name },
            Repr::Legacy(text) => Self::bare(text),
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn displays_qualified_and_bare() {
        assert_eq!(
            ObjectRef::qualified("public", "users").to_string(),
            "public.users"
        );
        assert_eq!(
            ObjectRef::bare("system.profile").to_string(),
            "system.profile"
        );
    }

    #[test]
    fn serde_round_trips_and_reads_legacy_strings() {
        let r = ObjectRef::qualified("public", "a.b");
        let json = serde_json::to_string(&r).unwrap();
        assert_eq!(json, r#"{"schema":"public","name":"a.b"}"#);
        assert_eq!(serde_json::from_str::<ObjectRef>(&json).unwrap(), r);
        assert_eq!(
            serde_json::from_str::<ObjectRef>(r#""public.users""#).unwrap(),
            ObjectRef::bare("public.users")
        );
    }

    #[test]
    fn legacy_qualified_splits_at_last_dot() {
        assert_eq!(
            ObjectRef::from_legacy_qualified("public.users", "public"),
            ObjectRef::qualified("public", "users")
        );
        assert_eq!(
            ObjectRef::from_legacy_qualified("users", "public"),
            ObjectRef::qualified("public", "users")
        );
    }
}
