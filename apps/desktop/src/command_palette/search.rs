use std::collections::HashSet;

use gpui_kit::{App, Entity};

use crate::app::prefs::manual_update_checks_enabled;
use crate::connection::ConnectionId;
use crate::workspace::TabSpec;
use crate::workspace::Workspaces;
use crate::workspace::connection_tree::ConnectionTree;
use crate::workspace::project_query::target_hint;

use super::types::{PaletteEvent, PaletteResult, PaletteSection, WorkspacePaletteAction};

pub struct SearchContext<'a> {
    pub connection_tree: &'a Entity<ConnectionTree>,
}

pub fn collect_sections(ctx: SearchContext<'_>, query: &str, cx: &App) -> Vec<PaletteSection> {
    let q = query.to_lowercase();
    let mut commands = vec![];
    let mut schema = vec![];
    let mut queries = vec![];
    let mut history = vec![];
    push_workspace_commands(&mut commands, &q, cx);
    push_schema_objects(&mut schema, ctx.connection_tree, &q, cx);
    push_saved_queries(&mut queries, &q, cx);
    push_history(&mut history, &q, cx);
    sections_from_parts(commands, schema, queries, history)
}

pub fn sections_from_parts(
    commands: Vec<PaletteResult>,
    schema: Vec<PaletteResult>,
    queries: Vec<PaletteResult>,
    history: Vec<PaletteResult>,
) -> Vec<PaletteSection> {
    [
        ("Commands", commands),
        ("Schema", schema),
        ("Queries", queries),
        ("History", history),
    ]
    .into_iter()
    .filter(|(_, items)| !items.is_empty())
    .map(|(heading, items)| PaletteSection { heading, items })
    .collect()
}

pub fn item_at(sections: &[PaletteSection], section: usize, row: usize) -> Option<&PaletteResult> {
    sections.get(section)?.items.get(row)
}

fn blank_command(action: WorkspacePaletteAction, label: &str, sublabel: &str) -> PaletteResult {
    PaletteResult {
        label: label.into(),
        sublabel: sublabel.into(),
        conn_label: String::new(),
        action: PaletteEvent::WorkspaceAction(action),
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
        if include_close_project_command(
            q,
            Workspaces::active_project(cx).is_some_and(|p| p.is_open()),
        ) {
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
            label: display,
            sublabel: format!("{} · {}", obj.kind.group(), conn_id),
            conn_label: String::new(),
            action: PaletteEvent::OpenTab(TabSpec::DataViewer {
                conn_id: conn_id.clone(),
                object: obj.object_ref(),
                filter: None,
            }),
        });
    }
}

fn push_saved_queries(results: &mut Vec<PaletteResult>, q: &str, cx: &App) {
    let Some(store) = Workspaces::active_project(cx).map(|p| &p.queries) else {
        return;
    };
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
                label: query.name.clone(),
                sublabel: format!("query · {target}"),
                conn_label: String::new(),
                action: PaletteEvent::OpenProjectQuery(query.path.clone()),
            });
        }
    }
}

fn push_history(results: &mut Vec<PaletteResult>, q: &str, cx: &App) {
    let Some(store) = Workspaces::active_project(cx).map(|p| &p.queries) else {
        return;
    };
    let mut seen_history: HashSet<(ConnectionId, String)> = HashSet::new();
    for entry in store.history.recent(100) {
        if q.is_empty() || entry.query.to_lowercase().contains(q) {
            let key = (entry.conn_id.clone(), entry.query.trim().to_lowercase());
            if !seen_history.insert(key) {
                continue;
            }
            results.push(PaletteResult {
                label: super::format::palette_single_line(&entry.query, 120),
                sublabel: format!("history · {}", entry.conn_id),
                conn_label: String::new(),
                action: PaletteEvent::InjectSql {
                    conn_id: entry.conn_id.clone(),
                    sql: entry.query.clone(),
                },
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

    fn stub_result(label: &str) -> PaletteResult {
        PaletteResult {
            label: label.into(),
            sublabel: String::new(),
            conn_label: String::new(),
            action: PaletteEvent::OpenTab(TabSpec::Home),
        }
    }

    #[test]
    fn sections_omit_empty_groups_and_keep_index_paths_stable() {
        let sections = sections_from_parts(
            vec![stub_result("Show Home")],
            vec![],
            vec![stub_result("active")],
            vec![],
        );
        assert_eq!(
            sections
                .iter()
                .map(|section| section.heading)
                .collect::<Vec<_>>(),
            ["Commands", "Queries"]
        );
        assert_eq!(
            item_at(&sections, 0, 0).map(|item| item.label.as_str()),
            Some("Show Home")
        );
        assert_eq!(
            item_at(&sections, 1, 0).map(|item| item.label.as_str()),
            Some("active")
        );
        assert!(item_at(&sections, 2, 0).is_none());
    }
}
