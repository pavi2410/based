//! Panel construction for PostgreSQL tabs.
//!
//! [`build_panel`] is the single dispatch point for all Postgres tab types.
//! Add new Postgres tabs here; `dispatch_open_tab` in workspace does not need
//! to change.

use std::sync::Arc;

use based_core::ObjectRef;
use gpui_kit::component::dock::PanelView;
use gpui_kit::{Context, Window, prelude::*};
use sqlx::PgPool;

use super::inspect::PgInspect;
use super::source::PgSource;
use crate::connection::ConnectionId;
use crate::workspace::Workspace;
use crate::workspace::panels::data_viewer::DataViewerPanel;
use crate::workspace::panels::query_editor::QueryEditorPanel;
use crate::workspace::panels::table_inspector::TableInspectorPanel;
use crate::workspace::tabs::label::tab_label_for_spec;
use crate::workspace::tabs::spec::{QueryEditorInit, TabSpec};

/// Try to build a Postgres panel for `spec`.
///
/// Returns `None` for tab kinds this engine doesn't handle.
pub fn build_panel(
    spec: &TabSpec,
    pool: PgPool,
    conn_id: &ConnectionId,
    window: &mut Window,
    cx: &mut Context<Workspace>,
) -> Option<Arc<dyn PanelView>> {
    match spec {
        TabSpec::DataViewer { object, filter, .. } => {
            // The tree always sets the schema. A spec without one is a legacy
            // "schema.name" string that restore could not migrate.
            let object = match object.schema {
                Some(_) => object.clone(),
                None => ObjectRef::from_legacy_qualified(&object.name, "public"),
            };
            let (schema, name) = (object.schema.unwrap_or_default(), object.name);
            let label = tab_label_for_spec(spec, false);
            let panel = cx.new(|cx| {
                DataViewerPanel::new(
                    Arc::new(PgSource {
                        pool,
                        schema,
                        table: name,
                    }),
                    conn_id.clone(),
                    filter.clone(),
                    window,
                    cx,
                )
            });
            panel.update(cx, |p, _| p.tab_label = label);
            Some(Arc::new(panel))
        }
        TabSpec::QueryEditor {
            init:
                QueryEditorInit {
                    sql,
                    auto_run,
                    saved,
                    dirty,
                },
            ..
        } => {
            let label = tab_label_for_spec(spec, false);
            let panel = cx.new(|cx| {
                QueryEditorPanel::new_with_initial(
                    Arc::new(super::query::PgQuery { pool }),
                    conn_id.clone(),
                    sql.clone(),
                    *auto_run,
                    window,
                    cx,
                )
            });
            panel.update(cx, |p, _| {
                p.tab_label = label;
                p.restore_saved_state(saved.as_ref(), *dirty);
            });
            Some(Arc::new(panel))
        }
        TabSpec::Inspector { object, .. } => {
            // The tree always sets the schema. A spec without one is a legacy
            // "schema.name" string that restore could not migrate.
            let object = match object.schema {
                Some(_) => object.clone(),
                None => ObjectRef::from_legacy_qualified(&object.name, "public"),
            };
            let (schema, name) = (object.schema.unwrap_or_default(), object.name);
            let label = tab_label_for_spec(spec, false);
            let panel = cx.new(|cx| {
                TableInspectorPanel::new(
                    Arc::new(PgInspect {
                        pool,
                        schema,
                        table: name,
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
