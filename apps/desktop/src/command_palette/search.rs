use std::collections::HashSet;

use gpui::{App, Entity};

use crate::connection::registry::ConnectionRegistry;
use crate::connection::{ConnectionId, EngineKind};
use crate::query_store::QueryStore;
use crate::workspace::connection_tree::ConnectionTree;
use crate::workspace::tab_spec::TabSpec;

use super::types::{PaletteResult, ResultKind, WorkspacePaletteAction};

pub struct SearchContext<'a> {
    pub registry: &'a Entity<ConnectionRegistry>,
    pub connection_tree: &'a Entity<ConnectionTree>,
}

pub fn collect_results(ctx: SearchContext<'_>, query: &str, cx: &App) -> Vec<PaletteResult> {
    let q = query.to_lowercase();
    let mut results = vec![];
    push_workspace_commands(&mut results, &q);
    push_schema_objects(&mut results, ctx.connection_tree, &q, cx);
    push_saved_queries(&mut results, &q, cx);
    push_history(&mut results, ctx.registry, &q, cx);
    results
}

fn blank_command(action: WorkspacePaletteAction, label: &str, sublabel: &str) -> PaletteResult {
    PaletteResult {
        kind: ResultKind::Command,
        label: label.into(),
        sublabel: sublabel.into(),
        conn_label: String::new(),
        spec: TabSpec::blank_query_editor(ConnectionId("".into())),
        command_action: Some(action),
        project_query_path: None,
    }
}

fn push_workspace_commands(results: &mut Vec<PaletteResult>, q: &str) {
    if q.is_empty() || q.contains("workspace") || q.contains("loose") || q.contains("collection") {
        results.push(blank_command(
            WorkspacePaletteAction::NewLooseQuery,
            "New loose query",
            "workspace",
        ));
        results.push(blank_command(
            WorkspacePaletteAction::NewCollection,
            "New collection",
            "workspace",
        ));
    }
    if q.is_empty() || q.contains("environment") || q.contains("no env") {
        results.push(blank_command(
            WorkspacePaletteAction::SelectNoEnvironment,
            "Select No Environment",
            "environment",
        ));
    }
    if q.is_empty() || q.contains("welcome") {
        results.push(blank_command(
            WorkspacePaletteAction::OpenWelcome,
            "Open Welcome",
            "navigation",
        ));
    }
    if q.is_empty() || q.contains("onboarding") || q.contains("setup") {
        results.push(blank_command(
            WorkspacePaletteAction::OpenOnboarding,
            "Open Onboarding",
            "navigation",
        ));
    }
    if q.is_empty() || q.contains("project") || q.contains("folder") || q.contains("open") {
        results.push(blank_command(
            WorkspacePaletteAction::OpenProject,
            "Open Project",
            "project",
        ));
        results.push(blank_command(
            WorkspacePaletteAction::OpenProjectInNewWindow,
            "Open Project in New Window",
            "project",
        ));
    }
    if crate::app::prefs::manual_update_checks_enabled() && (q.is_empty() || q.contains("update")) {
        results.push(blank_command(
            WorkspacePaletteAction::CheckForUpdates,
            "Check for Updates",
            "application",
        ));
    }
}

fn push_schema_objects(
    results: &mut Vec<PaletteResult>,
    connection_tree: &Entity<ConnectionTree>,
    q: &str,
    cx: &App,
) {
    let tree = connection_tree.read(cx);
    for (conn_id, obj, _engine) in tree.schema_palette_matches(q, cx) {
        let display = obj.display_name();
        results.push(PaletteResult {
            kind: ResultKind::SchemaObject,
            label: display.clone(),
            sublabel: format!("{} · {}", obj.kind.group(), conn_id.0),
            conn_label: String::new(),
            spec: TabSpec::DataViewer {
                conn_id: conn_id.clone(),
                object: display,
            },
            command_action: None,
            project_query_path: None,
        });
    }
}

fn push_saved_queries(results: &mut Vec<PaletteResult>, q: &str, cx: &App) {
    let store = cx.global::<QueryStore>();
    for query in store.project_queries() {
        let hay = format!(
            "{} {} {}",
            query.name,
            query.description.as_deref().unwrap_or(""),
            query.tags.join(" ")
        )
        .to_lowercase();
        if q.is_empty() || hay.contains(q) {
            let target = crate::workspace::project_query::target_hint(&query.target);
            results.push(PaletteResult {
                kind: ResultKind::SavedQuery,
                label: query.name.clone(),
                sublabel: format!("query · {target}"),
                conn_label: String::new(),
                spec: TabSpec::Welcome,
                project_query_path: Some(query.path.clone()),
                command_action: None,
            });
        }
    }
}

fn push_history(
    results: &mut Vec<PaletteResult>,
    registry: &Entity<ConnectionRegistry>,
    q: &str,
    cx: &App,
) {
    let store = cx.global::<QueryStore>();
    let mut seen_history: HashSet<(ConnectionId, String)> = HashSet::new();
    for entry in store.history.recent(100) {
        if q.is_empty() || entry.query.to_lowercase().contains(q) {
            let key = (entry.conn_id.clone(), entry.query.trim().to_lowercase());
            if !seen_history.insert(key) {
                continue;
            }
            let engine = registry
                .read(cx)
                .get(&entry.conn_id, cx)
                .map(|e| e.read(cx).config.engine());
            let spec = match engine {
                Some(EngineKind::MongoDB) => TabSpec::QueryEditor {
                    conn_id: entry.conn_id.clone(),
                    initial_sql: None,
                    initial_pipeline: Some(entry.query.clone()),
                    mongo_collection: None,
                    auto_run: false,
                },
                _ => TabSpec::QueryEditor {
                    conn_id: entry.conn_id.clone(),
                    initial_sql: Some(entry.query.clone()),
                    initial_pipeline: None,
                    mongo_collection: None,
                    auto_run: false,
                },
            };
            results.push(PaletteResult {
                kind: ResultKind::History,
                label: super::format::palette_single_line(&entry.query, 120),
                sublabel: format!("history · {}", entry.conn_id.0),
                conn_label: String::new(),
                spec,
                command_action: None,
                project_query_path: None,
            });
        }
    }
}
