//! Infer [`TabSpec`] from live dock panels so `TabManager` tracks tabs not opened via dispatch.
//!
//! Only panels whose spec can change after opening (query editors) or that are
//! opened outside dispatch need inference. For everything else `None` means
//! "keep the spec the tab was opened with".

use std::sync::Arc;

use gpui_kit::App;
use gpui_kit::component::dock::PanelView;

use crate::connection::ConnectionId;
use crate::mongodb::pipeline_builder::PipelineBuilderPanel;
use crate::workspace::panels::ConnectionWizardPanel;
use crate::workspace::panels::object_info::ConnectionDashboardPanel;
use crate::workspace::panels::query_editor::QueryEditorPanel;
use crate::workspace::panels::release_notes::ReleaseNotesPanel;

use super::spec::{QueryEditorInit, TabSpec};

pub(crate) fn infer_tab_spec(panel: &Arc<dyn PanelView>, cx: &App) -> Option<TabSpec> {
    match panel.panel_name(cx) {
        "HomePanel" => Some(TabSpec::Home),
        "ConnectionDashboard" => panel
            .view()
            .downcast::<ConnectionDashboardPanel>()
            .ok()
            .map(|ent| TabSpec::Dashboard {
                conn_id: ent.read(cx).connection_id(cx),
            }),
        "PgQueryEditor" | "SqliteQueryEditor" => {
            panel.view().downcast::<QueryEditorPanel>().ok().map(|ent| {
                let panel = ent.read(cx);
                query_editor_spec(panel.connection_id().clone(), panel.current_sql(cx))
            })
        }
        "MongoPipelineBuilder" => panel
            .view()
            .downcast::<PipelineBuilderPanel>()
            .ok()
            .map(|ent| {
                let panel = ent.read(cx);
                TabSpec::Pipeline {
                    conn_id: panel.connection_id().clone(),
                    collection: panel.pipeline_collection().to_string(),
                }
            }),
        "ReleaseNotesPanel" => panel
            .view()
            .downcast::<ReleaseNotesPanel>()
            .ok()
            .map(|ent| TabSpec::ReleaseNotes {
                version: ent.read(cx).version_label(cx),
            }),
        "ConnectionWizard" => panel
            .view()
            .downcast::<ConnectionWizardPanel>()
            .ok()
            .map(|ent| TabSpec::Builtin {
                conn_id: ent.read(cx).editing_id().cloned(),
                panel: "ConnectionWizard".into(),
            }),
        _ => None,
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

/// Placeholder spec for a panel nothing else identified.
pub(crate) fn fallback_spec(panel: &Arc<dyn PanelView>, cx: &App) -> TabSpec {
    TabSpec::Builtin {
        conn_id: None,
        panel: panel.panel_name(cx).to_string(),
    }
}
