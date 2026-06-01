use gpui::Context;

use crate::workspace::{QueryEditorInit, TabSpec};

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
            };
            if let Some(conn_id) = entry.spec.conn_id().cloned() {
                cx.emit(PaletteEvent::InjectSql { conn_id, sql });
            }
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
                    if let Some(conn_id) = entry.spec.conn_id().cloned() {
                        let table = entry.label.clone();
                        TabSpec::QueryEditor {
                            conn_id,
                            init: QueryEditorInit::Sql {
                                sql: Some(format!("SELECT * FROM {table} LIMIT 100")),
                                auto_run: false,
                            },
                        }
                    } else {
                        entry.spec.clone()
                    }
                }
                _ => entry.spec.clone(),
            };
            cx.emit(PaletteEvent::OpenTab(spec));
        }
    }
}
