use based_core::EngineKind;
use serde::Deserialize;

#[derive(Debug, Clone, Deserialize)]
pub struct QueryTarget {
    #[serde(default)]
    pub connection: Option<TargetConnection>,
    #[serde(default)]
    pub engine: Option<String>,
    #[serde(default)]
    pub tags: Vec<String>,
    #[serde(default, rename = "exclude_tags")]
    pub exclude_tags: Vec<String>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(untagged)]
pub enum TargetConnection {
    Exclusive(String),
    OneOf(Vec<String>),
}

impl QueryTarget {
    pub fn validate(&self) -> anyhow::Result<()> {
        if let Some(TargetConnection::OneOf(list)) = &self.connection
            && list.is_empty()
        {
            anyhow::bail!("[target].connection array must not be empty");
        }
        if let Some(TargetConnection::Exclusive(_)) = &self.connection {
            let has_filters =
                self.engine.is_some() || !self.tags.is_empty() || !self.exclude_tags.is_empty();
            if has_filters {
                anyhow::bail!(
                    "exclusive [target].connection string must not combine with engine, tags, or exclude_tags"
                );
            }
        }
        Ok(())
    }
}

#[derive(Debug, Clone)]
pub struct ConnectionRef {
    pub id: String,
    pub engine: EngineKind,
    pub tags: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ResolveError {
    ConnectionNotFound(String),
    NoMatches,
    Ambiguous(Vec<String>),
}

pub fn resolve_target(
    target: &QueryTarget,
    connections: &[ConnectionRef],
    focused: Option<&str>,
) -> Result<String, ResolveError> {
    if let Some(TargetConnection::Exclusive(id)) = &target.connection {
        if connections.iter().any(|c| c.id == *id) {
            return Ok(id.clone());
        }
        return Err(ResolveError::ConnectionNotFound(id.clone()));
    }

    let mut matches: Vec<&ConnectionRef> = connections.iter().collect();

    if let Some(engine) = &target.engine {
        let want = parse_engine(engine);
        matches.retain(|c| c.engine == want);
    }
    if !target.tags.is_empty() {
        matches.retain(|c| target.tags.iter().all(|t| c.tags.contains(t)));
    }
    if !target.exclude_tags.is_empty() {
        matches.retain(|c| !target.exclude_tags.iter().any(|t| c.tags.contains(t)));
    }
    if let Some(TargetConnection::OneOf(ids)) = &target.connection {
        matches.retain(|c| ids.contains(&c.id));
    }

    if matches.is_empty() {
        return Err(ResolveError::NoMatches);
    }

    if let Some(focus) = focused
        && let Some(found) = matches.iter().find(|c| c.id == focus)
    {
        return Ok(found.id.clone());
    }

    if matches.len() == 1 {
        return Ok(matches[0].id.clone());
    }

    Err(ResolveError::Ambiguous(
        matches.iter().map(|c| c.id.clone()).collect(),
    ))
}

fn parse_engine(s: &str) -> EngineKind {
    match s.to_lowercase().as_str() {
        "mongodb" | "mongo" => EngineKind::MongoDB,
        "sqlite" => EngineKind::SQLite,
        _ => EngineKind::Postgres,
    }
}

pub fn engine_kind_from_str(s: &str) -> EngineKind {
    parse_engine(s)
}
