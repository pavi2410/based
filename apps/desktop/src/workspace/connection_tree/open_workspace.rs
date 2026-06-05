use std::sync::Arc;

use ::mongodb::bson::Document;
use gpui::{Context, Window, prelude::*};
use gpui_component::dock::{DockItem, PanelView};

use crate::connection::AnyConnection;
use crate::postgres;
use crate::sqlite;
use crate::workspace::WorkspaceRef;

use super::super::dock_utils::wrap_center_root;
use super::ConnectionTree;

impl ConnectionTree {
    pub(crate) fn open_connected_workspace(
        &mut self,
        idx: usize,
        ac: &AnyConnection,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let Some(conn_ent) = self.registry.read(cx).connections().get(idx).cloned() else {
            return;
        };
        let (_label, _engine, conn_id) = {
            let entry = conn_ent.read(cx);
            (
                entry.config.label().to_string(),
                entry.config.engine(),
                entry.id.clone(),
            )
        };

        self.selected_connection = Some(idx);
        self.selected_object = None;
        self.set_connection_expanded(idx, true, cx);
        self.load_objects_for_connection(idx, ac.clone(), cx);

        let weak = self.dock_area.downgrade();
        let dashboard = cx.new(|cx| {
            crate::workspace::panels::object_info::ConnectionDashboardPanel::new(
                conn_ent.clone(),
                window,
                cx,
            )
        });

        let (center, panel_arcs): (DockItem, Vec<Arc<dyn PanelView>>) = match ac {
            AnyConnection::SQLite(ent) => {
                let pool = ent.read(cx).pool.clone();
                let query = cx.new(|cx| {
                    sqlite::query_editor::QueryEditorPanel::new(
                        pool.clone(),
                        conn_id.clone(),
                        window,
                        cx,
                    )
                });
                let pragma = cx.new(|cx| {
                    sqlite::pragma_browser::PragmaBrowserPanel::new(pool.clone(), window, cx)
                });
                let panels = vec![
                    Arc::new(dashboard) as Arc<dyn PanelView>,
                    Arc::new(query),
                    Arc::new(pragma),
                ];
                (
                    wrap_center_root(
                        DockItem::tabs(panels.clone(), &weak, window, cx),
                        &weak,
                        window,
                        cx,
                    ),
                    panels,
                )
            }
            AnyConnection::Postgres(ent) => {
                let pool = ent.read(cx).pool.clone();
                let query = cx.new(|cx| {
                    postgres::query_editor::QueryEditorPanel::new(
                        pool.clone(),
                        conn_id.clone(),
                        window,
                        cx,
                    )
                });
                let panels = vec![Arc::new(dashboard) as Arc<dyn PanelView>, Arc::new(query)];
                (
                    wrap_center_root(
                        DockItem::tabs(panels.clone(), &weak, window, cx),
                        &weak,
                        window,
                        cx,
                    ),
                    panels,
                )
            }
            AnyConnection::MongoDB(ent) => {
                let db = ent.read(cx).database().clone();
                let coll: ::mongodb::Collection<Document> = db.collection("based_explorer");
                let builder = cx.new(|cx| {
                    crate::mongodb::pipeline_builder::PipelineBuilderPanel::new(
                        coll.clone(),
                        conn_id.clone(),
                        window,
                        cx,
                    )
                });
                let stream = cx.new(|cx| {
                    crate::mongodb::change_stream::ChangeStreamPanel::new(coll, window, cx)
                });
                let panels = vec![
                    Arc::new(dashboard) as Arc<dyn PanelView>,
                    Arc::new(builder),
                    Arc::new(stream),
                ];
                (
                    wrap_center_root(
                        DockItem::tabs(panels.clone(), &weak, window, cx),
                        &weak,
                        window,
                        cx,
                    ),
                    panels,
                )
            }
        };

        // Replaces the Home tab with connection dashboards; Home returns when all DB tabs close.
        self.dock_area.update(cx, |dock, cx| {
            dock.set_center(center, window, cx);
        });

        if let Some(ws) = cx.try_global::<WorkspaceRef>().map(|w| w.0.clone()) {
            ws.update(cx, |ws, cx| {
                ws.replace_center_panels(panel_arcs, cx);
            });
        }
    }
}
