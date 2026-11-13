use based_project::{
    ConnectionRef, ProjectQuery, QueryBody, QueryTarget, ResolveError, resolve_target,
};

use crate::connection::ConnectionId;
use crate::connection::registry::ConnectionRegistry;
use crate::workspace::tabs::spec::SavedQueryRef;
use crate::workspace::{QueryEditorInit, TabSpec};
use based_core::ConnectionKey;

pub enum OpenQueryResult {
    Open(TabSpec),
    PickConnection {
        query_path: String,
        candidates: Vec<ConnectionId>,
    },
    Error(String),
}

pub fn open_project_query(
    query: &ProjectQuery,
    registry: &ConnectionRegistry,
    cx: &gpui_kit::App,
    focused: Option<&ConnectionId>,
) -> OpenQueryResult {
    let refs: Vec<ConnectionRef> = project_target_refs(registry, cx);

    let focused_key = focused.and_then(ConnectionId::project_key);
    let conn_id = match resolve_target(&query.target, &refs, focused_key.as_ref()) {
        Ok(id) => id,
        Err(ResolveError::Ambiguous(ids)) => {
            return OpenQueryResult::PickConnection {
                query_path: query.path.clone(),
                candidates: ids
                    .into_iter()
                    .map(|key| ConnectionId::project(&key))
                    .collect(),
            };
        }
        Err(ResolveError::NoMatches) => {
            return OpenQueryResult::Error("No connection matches this query's target.".into());
        }
        Err(ResolveError::ConnectionNotFound(id)) => {
            return OpenQueryResult::Error(format!("Connection not found: {id}"));
        }
    };

    OpenQueryResult::Open(tab_spec_for_query(query, ConnectionId::project(&conn_id)))
}

pub fn tab_spec_for_query(query: &ProjectQuery, conn_id: ConnectionId) -> TabSpec {
    let (query_path, name) = (&query.path, &query.name);
    match &query.body {
        QueryBody::Sql { query } => TabSpec::QueryEditor {
            conn_id,
            init: QueryEditorInit {
                sql: Some(query.clone()),
                auto_run: false,
                saved: Some(SavedQueryRef {
                    path: query_path.clone(),
                    name: name.clone(),
                }),
                dirty: false,
            },
        },
        QueryBody::Aggregate {
            pipeline,
            collection,
        } => TabSpec::Pipeline {
            conn_id,
            collection: collection.clone(),
            pipeline: Some(pipeline.clone()),
        },
    }
}

/// Whether this project query's `[target]` resolves to `conn_id` (including ambiguous matches).
pub fn query_targets_connection(
    query: &ProjectQuery,
    conn_id: &ConnectionId,
    registry: &ConnectionRegistry,
    cx: &gpui_kit::App,
) -> bool {
    let refs: Vec<ConnectionRef> = project_target_refs(registry, cx);

    let Some(key) = conn_id.project_key() else {
        return false;
    };
    match resolve_target(&query.target, &refs, Some(&key)) {
        Ok(id) => id == key,
        Err(ResolveError::Ambiguous(ids)) => ids.contains(&key),
        Err(_) => false,
    }
}

fn project_target_refs(registry: &ConnectionRegistry, cx: &gpui_kit::App) -> Vec<ConnectionRef> {
    registry
        .connections()
        .iter()
        .filter_map(|e| {
            let ent = e.read(cx);
            Some(ConnectionRef {
                id: ent.id.project_key()?,
                engine: ent.engine(),
                tags: ent.tags.clone(),
            })
        })
        .collect()
}

pub fn target_hint(target: &QueryTarget) -> String {
    use based_project::TargetConnection;
    match &target.connection {
        Some(TargetConnection::Exclusive(id)) => id.to_string(),
        Some(TargetConnection::OneOf(ids)) => ids
            .iter()
            .map(ConnectionKey::as_str)
            .collect::<Vec<_>>()
            .join(" | "),
        None => {
            let mut parts = Vec::new();
            if let Some(e) = target.engine {
                parts.push(e.to_string());
            }
            if !target.tags.is_empty() {
                parts.push(format!("tags:{}", target.tags.join("+")));
            }
            if parts.is_empty() {
                "any".into()
            } else {
                parts.join(", ")
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn only_project_ids_are_query_targets() {
        let project = ConnectionId::parse("local/northwind");
        let personal = ConnectionId::personal("local/northwind");
        assert_eq!(project.project_key(), Some("local/northwind".into()));
        assert_eq!(personal.project_key(), None);
        assert_eq!(ConnectionId::unsaved("aabb").project_key(), None);
    }
}
