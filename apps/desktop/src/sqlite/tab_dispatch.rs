//! Panel construction for SQLite tabs.
//!
//! [`build_panel`] is the single dispatch point for all SQLite tab types.
//! Add new SQLite tabs here; `dispatch_open_tab` in workspace does not need
//! to change.

use std::sync::Arc;

use gpui_kit::component::dock::PanelView;
use gpui_kit::{Context, Window, prelude::*};
use sqlx::SqlitePool;

use super::inspect::SqliteInspect;
use super::source::SqliteSource;
use crate::connection::ConnectionId;
use crate::workspace::Workspace;
use crate::workspace::panels::data_viewer::DataViewerPanel;
use crate::workspace::panels::table_inspector::TableInspectorPanel;
use crate::workspace::tabs::label::tab_label_for_spec;
use crate::workspace::tabs::spec::{QueryEditorInit, TabSpec};

/// Try to build a SQLite panel for `spec`.
///
/// Returns `None` for tab kinds this engine doesn't handle.
pub fn build_panel(
    spec: &TabSpec,
    pool: SqlitePool,
    conn_id: &ConnectionId,
    window: &mut Window,
    cx: &mut Context<Workspace>,
) -> Option<Arc<dyn PanelView>> {
    match spec {
        TabSpec::DataViewer { object, .. } => {
            let label = tab_label_for_spec(spec, false);
            let panel = cx.new(|cx| {
                DataViewerPanel::new(
                    Arc::new(SqliteSource {
                        pool,
                        table: object.clone(),
                    }),
                    conn_id.clone(),
                    window,
                    cx,
                )
            });
            panel.update(cx, |p, _| p.tab_label = label);
            Some(Arc::new(panel))
        }
        TabSpec::QueryEditor {
            init: QueryEditorInit::Sql { sql, auto_run },
            ..
        } => {
            let label = tab_label_for_spec(spec, false);
            let panel = cx.new(|cx| {
                super::query_editor::QueryEditorPanel::new_with_initial(
                    pool,
                    conn_id.clone(),
                    sql.clone(),
                    *auto_run,
                    window,
                    cx,
                )
            });
            panel.update(cx, |p, _| p.tab_label = label);
            Some(Arc::new(panel))
        }
        TabSpec::QueryEditor {
            init: QueryEditorInit::MongoPipeline { .. },
            ..
        } => None,
        TabSpec::Inspector { object, .. } => {
            let label = tab_label_for_spec(spec, false);
            let panel = cx.new(|cx| {
                TableInspectorPanel::new(
                    Arc::new(SqliteInspect {
                        pool,
                        table: object.clone(),
                    }),
                    conn_id.clone(),
                    window,
                    cx,
                )
            });
            panel.update(cx, |p, _| p.tab_label = label);
            Some(Arc::new(panel))
        }
        _ => None,
    }
}
