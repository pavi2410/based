use std::collections::HashSet;

use gpui::{App, Entity};

use crate::app::prefs::manual_update_checks_enabled;
use crate::connection::registry::ConnectionRegistry;
use crate::connection::{ConnectionId, EngineKind};
use crate::project::ProjectRoot;
use crate::query_store::QueryStore;
use crate::workspace::connection_tree::ConnectionTree;
use crate::workspace::project_query::target_hint;
use crate::workspace::{QueryEditorInit, TabSpec};

use super::types::{PaletteResult, ResultKind, WorkspacePaletteAction};

pub struct SearchContext<'a> {
    pub registry: &'a Entity<ConnectionRegistry>,
    pub connection_tree: &'a Entity<ConnectionTree>,
}

pub fn collect_results(ctx: SearchContext<'_>, query: &str, cx: &App) -> Vec<PaletteResult> {
    let q = query.to_lowercase();
    let mut results = vec![];
    push_workspace_commands(&mut results, &q, cx);
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

fn wants_project_commands(q: &str) -> bool {
    q.is_empty()
        || q.contains("project")
        || q.contains("folder")
        || q.contains("open")
        || q.contains("close")
}

fn include_close_project_command(q: &str, has_open_project: bool) -> bool {
    has_open_project && wants_project_commands(q)
}

fn wants_open_logs_command(q: &str) -> bool {
    q.is_empty() || q == "log" || q.contains("logs") || q.contains("open log")
}

fn push_workspace_commands(results: &mut Vec<PaletteResult>, q: &str, cx: &App) {
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
    if q.is_empty() || q.contains("home") {
        results.push(blank_command(
            WorkspacePaletteAction::OpenHome,
            "Show Home",
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
    if wants_open_logs_command(q) {
        results.push(blank_command(
            WorkspacePaletteAction::OpenLogs,
            "Open Logs",
            "application",
        ));
    }
    if wants_project_commands(q) {
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
        if include_close_project_command(q, cx.try_global::<ProjectRoot>().is_some()) {
            results.push(blank_command(
                WorkspacePaletteAction::CloseProject,
                "Close Project",
                "project",
            ));
        }
    }
    if manual_update_checks_enabled() && (q.is_empty() || q.contains("update")) {
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
            let target = target_hint(&query.target);
            results.push(PaletteResult {
                kind: ResultKind::SavedQuery,
                label: query.name.clone(),
                sublabel: format!("query · {target}"),
                conn_label: String::new(),
                spec: TabSpec::Home,
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
                    init: QueryEditorInit::MongoPipeline {
                        pipeline: Some(entry.query.clone()),
                        collection: None,
                    },
                },
                _ => TabSpec::QueryEditor {
                    conn_id: entry.conn_id.clone(),
                    init: QueryEditorInit::Sql {
                        sql: Some(entry.query.clone()),
                        auto_run: false,
                    },
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn close_query_matches_project_commands() {
        assert!(wants_project_commands("close"));
        assert!(wants_project_commands("project"));
        assert!(!wants_project_commands("update"));
    }

    #[test]
    fn close_project_command_only_when_a_project_is_open() {
        assert!(include_close_project_command("close", true));
        assert!(!include_close_project_command("close", false));
        assert!(!include_close_project_command("update", true));
    }

    #[test]
    fn open_logs_matches_log_queries_not_catalog() {
        assert!(wants_open_logs_command(""));
        assert!(wants_open_logs_command("log"));
        assert!(wants_open_logs_command("logs"));
        assert!(wants_open_logs_command("open logs"));
        assert!(!wants_open_logs_command("catalog"));
        assert!(!wants_open_logs_command("onboarding"));
    }
}
