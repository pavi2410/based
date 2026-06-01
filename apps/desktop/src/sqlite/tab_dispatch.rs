//! Panel construction for SQLite tabs.
//!
//! [`build_panel`] is the single dispatch point for all SQLite tab types.
//! Add new SQLite tabs here; `dispatch_open_tab` in workspace does not need
//! to change.

use std::sync::Arc;

use gpui::{Context, Window, prelude::*};
use gpui_component::dock::PanelView;
use sqlx::SqlitePool;

use crate::connection::ConnectionId;
use crate::workspace::Workspace;
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
                super::data_viewer::DataViewerPanel::new(pool, object.clone(), window, cx)
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
                super::inspector::TableInspectorPanel::new(pool, object.clone(), window, cx)
            });
            panel.update(cx, |p, _| p.tab_label = label);
            Some(Arc::new(panel))
        }
        _ => None,
    }
}
