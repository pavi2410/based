use gpui::Context;

use crate::workspace::{QueryEditorInit, TabSpec};

use super::CommandPalette;
use super::types::{PaletteEvent, PaletteResult, ResultKind};

pub fn event_for_selection(entry: &PaletteResult) -> Option<PaletteEvent> {
    match entry.kind {
        ResultKind::Command => entry
            .command_action
            .clone()
            .map(PaletteEvent::WorkspaceAction),
        ResultKind::History => {
            let sql = history_sql(entry);
            entry
                .spec
                .conn_id()
                .cloned()
                .map(|conn_id| PaletteEvent::InjectSql { conn_id, sql })
        }
        ResultKind::SavedQuery => Some(entry.project_query_path.as_ref().map_or_else(
            || PaletteEvent::OpenTab(entry.spec.clone()),
            |path| PaletteEvent::OpenProjectQuery(path.clone()),
        )),
        ResultKind::SchemaObject => Some(PaletteEvent::OpenTab(entry.spec.clone())),
    }
}

pub fn emit_selection(entry: &PaletteResult, cx: &mut Context<CommandPalette>) {
    if let Some(event) = event_for_selection(entry) {
        cx.emit(event);
    }
}

fn history_sql(entry: &PaletteResult) -> String {
    match &entry.spec {
        TabSpec::QueryEditor {
            init: QueryEditorInit::Sql { sql: Some(s), .. },
            ..
        } => s.clone(),
        TabSpec::QueryEditor {
            init:
                QueryEditorInit::MongoPipeline {
                    pipeline: Some(p), ..
                },
            ..
        } => p.clone(),
        _ => entry.label.clone(),
    }
}

#[cfg(test)]
mod tests {
    use crate::connection::ConnectionId;
    use crate::workspace::{QueryEditorInit, TabSpec};

    use super::super::types::{PaletteEvent, PaletteResult, ResultKind, WorkspacePaletteAction};
    use super::event_for_selection;

    fn conn(id: &str) -> ConnectionId {
        ConnectionId(id.into())
    }

    fn result(kind: ResultKind) -> PaletteResult {
        PaletteResult {
            kind,
            label: "users".into(),
            sublabel: "table · local".into(),
            conn_label: String::new(),
            spec: TabSpec::DataViewer {
                conn_id: conn("local"),
                object: "users".into(),
            },
            project_query_path: None,
            command_action: None,
        }
    }

    #[test]
    fn schema_object_opens_the_data_viewer() {
        let event = event_for_selection(&result(ResultKind::SchemaObject)).unwrap();
        assert_eq!(
            event,
            PaletteEvent::OpenTab(TabSpec::DataViewer {
                conn_id: conn("local"),
                object: "users".into(),
            })
        );
    }

    #[test]
    fn history_injects_sql_into_the_matching_connection() {
        let entry = PaletteResult {
            kind: ResultKind::History,
            label: "SELECT 1".into(),
            sublabel: "history · local".into(),
            conn_label: String::new(),
            spec: TabSpec::QueryEditor {
                conn_id: conn("local"),
                init: QueryEditorInit::Sql {
                    sql: Some("SELECT 1".into()),
                    auto_run: false,
                },
            },
            project_query_path: None,
            command_action: None,
        };
        assert_eq!(
            event_for_selection(&entry).unwrap(),
            PaletteEvent::InjectSql {
                conn_id: conn("local"),
                sql: "SELECT 1".into(),
            }
        );
    }

    #[test]
    fn saved_query_opens_the_project_path() {
        let mut entry = result(ResultKind::SavedQuery);
        entry.label = "active users".into();
        entry.project_query_path = Some("queries/active.sql".into());
        assert_eq!(
            event_for_selection(&entry).unwrap(),
            PaletteEvent::OpenProjectQuery("queries/active.sql".into())
        );
    }

    #[test]
    fn command_emits_the_workspace_action() {
        let mut entry = result(ResultKind::Command);
        entry.command_action = Some(WorkspacePaletteAction::OpenHome);
        assert_eq!(
            event_for_selection(&entry).unwrap(),
            PaletteEvent::WorkspaceAction(WorkspacePaletteAction::OpenHome)
        );
    }
}
