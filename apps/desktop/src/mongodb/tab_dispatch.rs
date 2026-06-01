//! Panel construction for MongoDB tabs.
//!
//! [`build_panel`] is the single dispatch point for all MongoDB tab types.
//! Add new MongoDB tabs here; `dispatch_open_tab` in workspace does not need
//! to change.

use std::sync::Arc;

use ::mongodb::bson::Document;
use gpui::{Context, Window, prelude::*};
use gpui_component::dock::PanelView;
use mongodb::{Collection, Database};

use crate::connection::ConnectionId;
use crate::workspace::Workspace;
use crate::workspace::tabs::label::tab_label_for_spec;
use crate::workspace::tabs::spec::{QueryEditorInit, TabSpec};

/// Try to build a MongoDB panel for `spec`.
///
/// Returns `None` for tab kinds this engine doesn't handle.
pub fn build_panel(
    spec: &TabSpec,
    db: Database,
    conn_id: &ConnectionId,
    window: &mut Window,
    cx: &mut Context<Workspace>,
) -> Option<Arc<dyn PanelView>> {
    match spec {
        TabSpec::DataViewer { object, .. } => {
            let label = tab_label_for_spec(spec, false);
            let collection: Collection<Document> = db.collection(object);
            let panel = cx
                .new(|cx| super::document_viewer::DocumentViewerPanel::new(collection, window, cx));
            panel.update(cx, |p, _| p.tab_label = label);
            Some(Arc::new(panel))
        }
        TabSpec::QueryEditor {
            init:
                QueryEditorInit::MongoPipeline {
                    pipeline,
                    collection,
                },
            ..
        } => {
            let label = tab_label_for_spec(spec, false);
            let coll_name = collection
                .as_deref()
                .map(str::trim)
                .filter(|s| !s.is_empty())
                .unwrap_or("based_explorer");
            let coll: Collection<Document> = db.collection(coll_name);
            let panel = cx.new(|cx| {
                super::pipeline_builder::PipelineBuilderPanel::new_with_pipeline(
                    coll,
                    conn_id.clone(),
                    pipeline.clone(),
                    window,
                    cx,
                )
            });
            panel.update(cx, |p, _| p.tab_label = label);
            Some(Arc::new(panel))
        }
        TabSpec::QueryEditor {
            init: QueryEditorInit::Sql { .. },
            ..
        } => None,
        TabSpec::Pipeline { collection, .. } => {
            let label = tab_label_for_spec(spec, false);
            let coll: Collection<Document> = db.collection(collection);
            let panel = cx.new(|cx| {
                super::pipeline_builder::PipelineBuilderPanel::new(
                    coll,
                    conn_id.clone(),
                    window,
                    cx,
                )
            });
            panel.update(cx, |p, _| p.tab_label = label);
            Some(Arc::new(panel))
        }
        TabSpec::Inspector { object, .. } => {
            let label = tab_label_for_spec(spec, false);
            let coll: Collection<Document> = db.collection(object);
            let panel =
                cx.new(|cx| super::inspector::CollectionInspectorPanel::new(coll, window, cx));
            panel.update(cx, |p, _| p.tab_label = label);
            Some(Arc::new(panel))
        }
        TabSpec::DocumentInsert { collection, .. } => {
            let label = tab_label_for_spec(spec, false);
            let coll: Collection<Document> = db.collection(collection);
            let panel = cx.new(|cx| {
                super::document_editor::DocumentEditorPanel::new_insert(coll, window, cx)
            });
            panel.update(cx, |p, _| p.tab_label = label);
            Some(Arc::new(panel))
        }
        _ => None,
    }
}
