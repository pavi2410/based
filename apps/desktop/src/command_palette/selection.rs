use gpui::Context;

use crate::workspace::tab_spec::TabSpec;

use super::CommandPalette;
use super::types::{PaletteEvent, PaletteResult, ResultKind};

pub fn emit_selection(entry: &PaletteResult, secondary: bool, cx: &mut Context<CommandPalette>) {
    match (&entry.kind, secondary) {
        (ResultKind::Command, _) => {
            if let Some(action) = entry.command_action.clone() {
                cx.emit(PaletteEvent::WorkspaceAction(action));
            }
        }
        (ResultKind::History, false) => {
            let sql = match &entry.spec {
                TabSpec::QueryEditor {
                    initial_sql: Some(s),
                    ..
                } => s.clone(),
                TabSpec::QueryEditor {
                    initial_pipeline: Some(p),
                    ..
                } => p.clone(),
                _ => entry.label.clone(),
            };
            cx.emit(PaletteEvent::InjectSql {
                conn_id: entry.spec.conn_id().clone(),
                sql,
            });
        }
        (ResultKind::SavedQuery, _) => {
            if let Some(path) = &entry.project_query_path {
                cx.emit(PaletteEvent::OpenProjectQuery(path.clone()));
            } else {
                cx.emit(PaletteEvent::OpenTab(entry.spec.clone()));
            }
        }
        _ => {
            let spec = match (&entry.kind, secondary) {
                (ResultKind::SchemaObject, true) => {
                    let table = entry.label.clone();
                    TabSpec::QueryEditor {
                        conn_id: entry.spec.conn_id().clone(),
                        initial_sql: Some(format!("SELECT * FROM {table} LIMIT 100")),
                        initial_pipeline: None,
                        mongo_collection: None,
                        auto_run: false,
                    }
                }
                _ => entry.spec.clone(),
            };
            cx.emit(PaletteEvent::OpenTab(spec));
        }
    }
}
