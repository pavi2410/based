use based_project::{
    ConnectionRef, ProjectQuery, QueryBody, QueryTarget, ResolveError, resolve_target,
};

use crate::connection::ConnectionId;
use crate::connection::registry::ConnectionRegistry;
use crate::workspace::{QueryEditorInit, TabSpec};

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
    cx: &gpui::App,
    focused: Option<&ConnectionId>,
) -> OpenQueryResult {
    let refs: Vec<ConnectionRef> = registry
        .connections()
        .iter()
        .map(|e| {
            let ent = e.read(cx);
            ConnectionRef {
                id: ent.id.0.clone(),
                engine: ent.engine(),
                tags: ent.tags.clone(),
            }
        })
        .collect();

    let focused_key = focused.map(|id| id.0.as_str());
    let conn_id = match resolve_target(&query.target, &refs, focused_key) {
        Ok(id) => id,
        Err(ResolveError::Ambiguous(ids)) => {
            return OpenQueryResult::PickConnection {
                query_path: query.path.clone(),
                candidates: ids
                    .into_iter()
                    .map(|id| ConnectionId::from_key(&id))
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

    OpenQueryResult::Open(tab_spec_for_query(query, ConnectionId::from_key(&conn_id)))
}

pub fn tab_spec_for_query(query: &ProjectQuery, conn_id: ConnectionId) -> TabSpec {
    match &query.body {
        QueryBody::Sql { query } => TabSpec::QueryEditor {
            conn_id,
            init: QueryEditorInit::Sql {
                sql: Some(query.clone()),
                auto_run: false,
            },
        },
        QueryBody::Aggregate {
            pipeline,
            collection,
        } => TabSpec::QueryEditor {
            conn_id,
            init: QueryEditorInit::MongoPipeline {
                pipeline: Some(pipeline.clone()),
                collection: collection.clone(),
            },
        },
    }
}

pub fn target_hint(target: &QueryTarget) -> String {
    use based_project::TargetConnection;
    match &target.connection {
        Some(TargetConnection::Exclusive(id)) => id.clone(),
        Some(TargetConnection::OneOf(ids)) => ids.join(" | "),
        None => {
            let mut parts = Vec::new();
            if let Some(e) = &target.engine {
                parts.push(e.clone());
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
