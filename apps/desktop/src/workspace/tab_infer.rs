//! Infer [`TabSpec`] from live dock panels so `TabManager` tracks tabs not opened via dispatch.

use std::sync::Arc;

use gpui::App;
use gpui_component::dock::PanelView;

use crate::mongodb::pipeline_builder::PipelineBuilderPanel;
use crate::postgres;
use crate::sqlite;
use crate::workspace::object_info::ConnectionDashboardPanel;

use super::tab_spec::TabSpec;

pub(crate) fn infer_tab_spec(panel: &Arc<dyn PanelView>, cx: &App) -> TabSpec {
    match panel.panel_name(cx) {
        "WelcomePanel" => TabSpec::Welcome,
        "ConnectionDashboard" => panel
            .view()
            .downcast::<ConnectionDashboardPanel>()
            .map(|ent| TabSpec::Dashboard(ent.read(cx).connection_id(cx)))
            .unwrap_or_else(|_| builtin(panel, cx)),
        "PgQueryEditor" => panel
            .view()
            .downcast::<postgres::query_editor::QueryEditorPanel>()
            .map(|ent| TabSpec::blank_query_editor(ent.read(cx).connection_id().clone()))
            .unwrap_or_else(|_| builtin(panel, cx)),
        "SqliteQueryEditor" => panel
            .view()
            .downcast::<sqlite::query_editor::QueryEditorPanel>()
            .map(|ent| TabSpec::blank_query_editor(ent.read(cx).connection_id().clone()))
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
        _ => builtin(panel, cx),
    }
}

fn builtin(panel: &Arc<dyn PanelView>, cx: &App) -> TabSpec {
    TabSpec::Builtin {
        conn_id: None,
        panel: panel.panel_name(cx).to_string(),
    }
}
