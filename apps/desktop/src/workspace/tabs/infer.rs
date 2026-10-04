//! Infer [`TabSpec`] from live dock panels so `TabManager` tracks tabs not opened via dispatch.

use std::sync::Arc;

use gpui_kit::App;
use gpui_kit::component::dock::PanelView;

use crate::connection::ConnectionId;
use crate::mongodb::pipeline_builder::PipelineBuilderPanel;
use crate::postgres;
use crate::sqlite;
use crate::workspace::panels::ConnectionWizardPanel;
use crate::workspace::panels::object_info::ConnectionDashboardPanel;
use crate::workspace::panels::release_notes::ReleaseNotesPanel;

use super::spec::{QueryEditorInit, TabSpec};

pub(crate) fn infer_tab_spec(panel: &Arc<dyn PanelView>, cx: &App) -> TabSpec {
    match panel.panel_name(cx) {
        "HomePanel" => TabSpec::Home,
        "ConnectionDashboard" => panel
            .view()
            .downcast::<ConnectionDashboardPanel>()
            .map(|ent| TabSpec::Dashboard {
                conn_id: ent.read(cx).connection_id(cx),
            })
            .unwrap_or_else(|_| builtin(panel, cx)),
        "PgQueryEditor" => panel
            .view()
            .downcast::<postgres::query_editor::QueryEditorPanel>()
            .map(|ent| {
                let panel = ent.read(cx);
                query_editor_spec(panel.connection_id().clone(), panel.current_sql(cx))
            })
            .unwrap_or_else(|_| builtin(panel, cx)),
        "SqliteQueryEditor" => panel
            .view()
            .downcast::<sqlite::query_editor::QueryEditorPanel>()
            .map(|ent| {
                let panel = ent.read(cx);
                query_editor_spec(panel.connection_id().clone(), panel.current_sql(cx))
            })
            .unwrap_or_else(|_| builtin(panel, cx)),
        "MongoPipelineBuilder" => panel
            .view()
            .downcast::<PipelineBuilderPanel>()
            .map(|ent| {
                let panel = ent.read(cx);
                TabSpec::Pipeline {
                    conn_id: panel.connection_id().clone(),
                    collection: panel.pipeline_collection().to_string(),
                }
            })
            .unwrap_or_else(|_| builtin(panel, cx)),
        "ReleaseNotesPanel" => panel
            .view()
            .downcast::<ReleaseNotesPanel>()
            .map(|ent| TabSpec::ReleaseNotes {
                version: ent.read(cx).version_label(cx),
            })
            .unwrap_or_else(|_| builtin(panel, cx)),
        "ConnectionWizard" => panel
            .view()
            .downcast::<ConnectionWizardPanel>()
            .map(|ent| TabSpec::Builtin {
                conn_id: ent.read(cx).editing_id().cloned(),
                panel: "ConnectionWizard".into(),
            })
            .unwrap_or_else(|_| builtin(panel, cx)),
        _ => builtin(panel, cx),
    }
}

fn query_editor_spec(conn_id: ConnectionId, sql: String) -> TabSpec {
    TabSpec::QueryEditor {
        conn_id,
        init: QueryEditorInit::Sql {
            sql: (!sql.is_empty()).then_some(sql),
            auto_run: false,
        },
    }
}

fn builtin(panel: &Arc<dyn PanelView>, cx: &App) -> TabSpec {
    TabSpec::Builtin {
        conn_id: None,
        panel: panel.panel_name(cx).to_string(),
    }
}
