use std::fmt::{self, Display, Formatter};
use std::sync::Arc;

use serde::{Deserialize, Deserializer, Serialize, Serializer};

/// A connection's id inside a based-dir: its path under `connections/` without
/// the `.toml` suffix (`local/northwind`). This is what committed query files
/// name in `[target].connection`; it carries no scope.
#[derive(Debug, Clone, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(transparent)]
pub struct ConnectionKey(String);

impl ConnectionKey {
    pub fn new(key: impl Into<String>) -> Self {
        Self(key.into())
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl Display for ConnectionKey {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

impl PartialEq<str> for ConnectionKey {
    fn eq(&self, other: &str) -> bool {
        self.0 == other
    }
}

impl PartialEq<&str> for ConnectionKey {
    fn eq(&self, other: &&str) -> bool {
        self.0 == *other
    }
}

impl From<&str> for ConnectionKey {
    fn from(s: &str) -> Self {
        Self(s.to_string())
    }
}

impl From<String> for ConnectionKey {
    fn from(s: String) -> Self {
        Self(s)
    }
}

/// Where a connection lives.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ConnectionScope {
    /// `.based/connections/` of the open project.
    Project,
    /// Personal based-dir (`~/.config/based`).
    Personal,
    /// Live session never written to disk.
    Unsaved,
    /// User-local wizard template.
    Template,
}

impl ConnectionScope {
    /// Prefix of the persisted text form (`user:…`); empty for project ids.
    const fn prefix(self) -> &'static str {
        match self {
            Self::Project => "",
            Self::Personal => "user:",
            Self::Unsaved => "unsaved:",
            Self::Template => "ws-template:",
        }
    }
}

/// A connection in the registry: its scope plus its key within that scope.
///
/// The text form (`Display`, serde) is the key with a scope prefix
/// (`user:local/northwind`, `unsaved:…`, `ws-template:…`; project ids are bare),
/// the same strings sessions and the metadata store already hold.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct ConnectionId {
    scope: ConnectionScope,
    key: Arc<str>,
}

impl ConnectionId {
    pub fn new(scope: ConnectionScope, key: &str) -> Self {
        Self {
            scope,
            key: key.into(),
        }
    }

    /// A connection in the open project's `.based/connections/`.
    pub fn project(key: &ConnectionKey) -> Self {
        Self::new(ConnectionScope::Project, key.as_str())
    }

    /// A connection in the personal based-dir.
    pub fn personal(key: &str) -> Self {
        Self::new(ConnectionScope::Personal, key)
    }

    /// A live wizard session that is not written to disk.
    pub fn unsaved(key: &str) -> Self {
        Self::new(ConnectionScope::Unsaved, key)
    }

    /// Read the text form back. Unprefixed text is a project id.
    pub fn parse(text: &str) -> Self {
        for scope in [
            ConnectionScope::Personal,
            ConnectionScope::Unsaved,
            ConnectionScope::Template,
        ] {
            if let Some(key) = text.strip_prefix(scope.prefix()) {
                return Self::new(scope, key);
            }
        }
        Self::new(ConnectionScope::Project, text)
    }

    pub fn scope(&self) -> ConnectionScope {
        self.scope
    }

    /// The key within its scope (no prefix).
    pub fn key(&self) -> &str {
        &self.key
    }

    /// The key a committed query file may name, for project connections only.
    pub fn project_key(&self) -> Option<ConnectionKey> {
        (self.scope == ConnectionScope::Project).then(|| ConnectionKey::new(&*self.key))
    }

    /// User-local connection stored in workspace metadata, not the open `.based/` project.
    pub fn is_workspace_local(&self) -> bool {
        self.scope != ConnectionScope::Project
    }

    /// Connection loaded from the personal based-dir.
    pub fn is_personal(&self) -> bool {
        self.scope == ConnectionScope::Personal
    }

    /// Live session that is not a project or personal file.
    pub fn is_unsaved(&self) -> bool {
        self.scope == ConnectionScope::Unsaved
    }

    /// Unsaved live sessions and workspace templates — never owned by a based-dir snapshot.
    pub fn is_ephemeral(&self) -> bool {
        matches!(
            self.scope,
            ConnectionScope::Unsaved | ConnectionScope::Template
        )
    }
}

impl Display for ConnectionId {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        f.write_str(self.scope.prefix())?;
        f.write_str(&self.key)
    }
}

impl Serialize for ConnectionId {
    fn serialize<S: Serializer>(&self, s: S) -> Result<S::Ok, S::Error> {
        s.collect_str(self)
    }
}

impl<'de> Deserialize<'de> for ConnectionId {
    fn deserialize<D: Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
        let text = String::deserialize(d)?;
        Ok(Self::parse(&text))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn text_form_round_trips() {
        for text in [
            "local/northwind",
            "user:local/northwind",
            "unsaved:aabb",
            "ws-template:aaaaaaaa-bbbb-cccc-dddd-eeeeeeeeeeee",
        ] {
            assert_eq!(ConnectionId::parse(text).to_string(), text);
        }
        let json = serde_json::to_string(&ConnectionId::personal("a/b")).unwrap();
        assert_eq!(json, r#""user:a/b""#);
        assert_eq!(
            serde_json::from_str::<ConnectionId>(&json).unwrap(),
            ConnectionId::personal("a/b")
        );
    }

    #[test]
    fn scopes() {
        let template = ConnectionId::parse("ws-template:aaaa");
        assert_eq!(template.scope(), ConnectionScope::Template);
        assert!(template.is_workspace_local() && template.is_ephemeral());

        let project = ConnectionId::project(&"local/northwind".into());
        assert!(!project.is_workspace_local() && !project.is_personal());
        assert_eq!(project.project_key(), Some("local/northwind".into()));

        let personal = ConnectionId::personal("local/northwind");
        assert_eq!(personal.key(), "local/northwind");
        assert!(personal.is_personal() && personal.is_workspace_local());
        assert!(!personal.is_ephemeral());
        assert_eq!(personal.project_key(), None);
        assert_ne!(personal, project);

        let unsaved = ConnectionId::unsaved("aabb");
        assert!(unsaved.is_unsaved() && unsaved.is_ephemeral());
        assert_eq!(unsaved.project_key(), None);
    }
}
