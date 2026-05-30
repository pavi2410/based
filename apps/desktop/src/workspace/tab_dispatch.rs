use std::sync::Arc;

use ::mongodb::bson::Document;
use gpui::{AnyView, App, Context, Entity, Window, prelude::*};
use gpui_component::dock::{DockPlacement, PanelView};
use mongodb::Collection;

use crate::connection::{AnyConnection, ConnectionEntry, ConnectionId, ConnectionState};
use crate::postgres;
use crate::sqlite;

use super::TabSpec;
use super::Workspace;
use super::object_info::ObjectInfoPanel;
use super::tab_label::tab_label_for_spec;

/// Set dock tab label from [`TabSpec`] then register the panel.
macro_rules! register_dock_panel {
    ($ws:expr, $spec:expr, $ent:expr, $window:expr, $cx:expr) => {{
        let label = tab_label_for_spec(&$spec, false);
        $ent.update($cx, |panel, _| {
            panel.tab_label = label;
        });
        let arc = std::sync::Arc::new($ent);
        $ws.dock_add_and_register_tab($spec, arc, $window, $cx);
    }};
}

impl Workspace {
    fn find_connection(&self, id: &ConnectionId, cx: &App) -> Option<Entity<ConnectionEntry>> {
        self.registry
            .read(cx)
            .connections()
            .iter()
            .find(|e| e.read(cx).id == *id)
            .cloned()
    }

    pub(crate) fn dispatch_open_tab(
        &mut self,
        spec: TabSpec,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        match spec {
            TabSpec::Dashboard(conn_id) => {
                self.connection_tree.update(cx, |tree, ecx| {
                    tree.focus_connection_by_id(&conn_id, ecx);
                });
            }
            TabSpec::DataViewer { conn_id, object } => {
                let Some(ent) = self.find_connection(&conn_id, cx) else {
                    return;
                };
                let ac = match &ent.read(cx).state {
                    ConnectionState::Connected(ac) => ac.clone(),
                    _ => return,
                };
                let tab_spec_for_manager = TabSpec::DataViewer {
                    conn_id: conn_id.clone(),
                    object: object.clone(),
                };
                match ac {
                    AnyConnection::SQLite(conn) => {
                        let pool = conn.read(cx).pool.clone();
                        let panel_ent = cx.new(|cx| {
                            sqlite::data_viewer::DataViewerPanel::new(pool, object, window, cx)
                        });
                        register_dock_panel!(self, tab_spec_for_manager, panel_ent, window, cx);
                    }
                    AnyConnection::Postgres(conn) => {
                        let pool = conn.read(cx).pool.clone();
                        let (schema, name) = match object.rsplit_once('.') {
                            Some((s, n)) if !n.is_empty() => (s.to_string(), n.to_string()),
                            _ => ("public".to_string(), object),
                        };
                        let panel_ent = cx.new(|cx| {
                            postgres::data_viewer::DataViewerPanel::new(
                                pool, schema, name, window, cx,
                            )
                        });
                        register_dock_panel!(self, tab_spec_for_manager, panel_ent, window, cx);
                    }
                    AnyConnection::MongoDB(conn) => {
                        let db = conn.read(cx).database().clone();
                        let collection: Collection<Document> = db.collection(&object);
                        let panel_ent = cx.new(|cx| {
                            crate::mongodb::document_viewer::DocumentViewerPanel::new(
                                collection, window, cx,
                            )
                        });
                        register_dock_panel!(self, tab_spec_for_manager, panel_ent, window, cx);
                    }
                }
            }
            TabSpec::QueryEditor {
                conn_id,
                initial_sql,
                initial_pipeline,
                auto_run,
                mongo_collection,
            } => {
                let Some(ent) = self.find_connection(&conn_id, cx) else {
                    return;
                };
                let ac = match &ent.read(cx).state {
                    ConnectionState::Connected(ac) => ac.clone(),
                    _ => return,
                };
                let tab_spec_for_manager = TabSpec::QueryEditor {
                    conn_id: conn_id.clone(),
                    initial_sql: initial_sql.clone(),
                    initial_pipeline: initial_pipeline.clone(),
                    auto_run,
                    mongo_collection: mongo_collection.clone(),
                };
                match ac {
                    AnyConnection::SQLite(conn) => {
                        let pool = conn.read(cx).pool.clone();
                        let panel_ent = cx.new(|cx| {
                            sqlite::query_editor::QueryEditorPanel::new_with_initial(
                                pool,
                                conn_id.clone(),
                                initial_sql.clone(),
                                auto_run,
                                window,
                                cx,
                            )
                        });
                        register_dock_panel!(self, tab_spec_for_manager, panel_ent, window, cx);
                    }
                    AnyConnection::Postgres(conn) => {
                        let pool = conn.read(cx).pool.clone();
                        let panel_ent = cx.new(|cx| {
                            postgres::query_editor::QueryEditorPanel::new_with_initial(
                                pool,
                                conn_id.clone(),
                                initial_sql.clone(),
                                auto_run,
                                window,
                                cx,
                            )
                        });
                        register_dock_panel!(self, tab_spec_for_manager, panel_ent, window, cx);
                    }
                    AnyConnection::MongoDB(conn) => {
                        let db = conn.read(cx).database().clone();
                        let coll_name = mongo_collection
                            .as_deref()
                            .map(str::trim)
                            .filter(|s| !s.is_empty())
                            .unwrap_or("based_explorer");
                        let coll: Collection<Document> = db.collection(coll_name);
                        let merged = initial_pipeline.clone().or(initial_sql.clone());
                        let panel_ent = cx.new(|cx| {
                            crate::mongodb::pipeline_builder::PipelineBuilderPanel::new_with_pipeline(
                                coll,
                                conn_id.clone(),
                                merged,
                                window,
                                cx,
                            )
                        });
                        register_dock_panel!(self, tab_spec_for_manager, panel_ent, window, cx);
                    }
                }
            }
            TabSpec::Pipeline {
                conn_id,
                collection,
            } => {
                let Some(ent) = self.find_connection(&conn_id, cx) else {
                    return;
                };
                let ac = match &ent.read(cx).state {
                    ConnectionState::Connected(ac) => ac.clone(),
                    _ => return,
                };
                let AnyConnection::MongoDB(conn) = ac else {
                    log::warn!("pipeline tab requires a MongoDB connection");
                    return;
                };
                let tab_spec_for_manager = TabSpec::Pipeline {
                    conn_id: conn_id.clone(),
                    collection: collection.clone(),
                };
                let db = conn.read(cx).database().clone();
                let coll: Collection<Document> = db.collection(&collection);
                let panel_ent = cx.new(|cx| {
                    crate::mongodb::pipeline_builder::PipelineBuilderPanel::new(
                        coll,
                        conn_id.clone(),
                        window,
                        cx,
                    )
                });
                register_dock_panel!(self, tab_spec_for_manager, panel_ent, window, cx);
            }
            TabSpec::Inspector { conn_id, object } => {
                let Some(ent) = self.find_connection(&conn_id, cx) else {
                    return;
                };
                let ac = match &ent.read(cx).state {
                    ConnectionState::Connected(ac) => ac.clone(),
                    _ => return,
                };
                let tab_spec_for_manager = TabSpec::Inspector {
                    conn_id: conn_id.clone(),
                    object: object.clone(),
                };
                match ac {
                    AnyConnection::SQLite(conn) => {
                        let pool = conn.read(cx).pool.clone();
                        let panel_ent = cx.new(|cx| {
                            sqlite::inspector::TableInspectorPanel::new(
                                pool,
                                object.clone(),
                                window,
                                cx,
                            )
                        });
                        register_dock_panel!(self, tab_spec_for_manager, panel_ent, window, cx);
                    }
                    AnyConnection::Postgres(conn) => {
                        let pool = conn.read(cx).pool.clone();
                        let (schema, name) = match object.rsplit_once('.') {
                            Some((s, n)) if !n.is_empty() => (s.to_string(), n.to_string()),
                            _ => ("public".to_string(), object.clone()),
                        };
                        let panel_ent = cx.new(|cx| {
                            postgres::inspector::TableInspectorPanel::new(
                                pool, schema, name, window, cx,
                            )
                        });
                        register_dock_panel!(self, tab_spec_for_manager, panel_ent, window, cx);
                    }
                    AnyConnection::MongoDB(conn) => {
                        let db = conn.read(cx).database().clone();
                        let coll: Collection<Document> = db.collection(&object);
                        let panel_ent = cx.new(|cx| {
                            crate::mongodb::inspector::CollectionInspectorPanel::new(
                                coll, window, cx,
                            )
                        });
                        register_dock_panel!(self, tab_spec_for_manager, panel_ent, window, cx);
                    }
                }
            }
            TabSpec::ObjectInfo {
                conn_id,
                object_name,
                kind_label,
            } => {
                let tab_spec_for_manager = TabSpec::ObjectInfo {
                    conn_id: conn_id.clone(),
                    object_name: object_name.clone(),
                    kind_label: kind_label.clone(),
                };
                let panel_ent = cx.new(|cx| {
                    ObjectInfoPanel::new(object_name.clone(), kind_label.clone(), window, cx)
                });
                register_dock_panel!(self, tab_spec_for_manager, panel_ent, window, cx);
            }
            TabSpec::DocumentInsert {
                conn_id,
                collection,
            } => {
                let Some(ent) = self.find_connection(&conn_id, cx) else {
                    return;
                };
                let ac = match &ent.read(cx).state {
                    ConnectionState::Connected(ac) => ac.clone(),
                    _ => return,
                };
                let AnyConnection::MongoDB(conn) = ac else {
                    log::warn!("document insert tab requires MongoDB");
                    return;
                };
                let tab_spec_for_manager = TabSpec::DocumentInsert {
                    conn_id: conn_id.clone(),
                    collection: collection.clone(),
                };
                let db = conn.read(cx).database().clone();
                let coll: Collection<Document> = db.collection(&collection);
                let panel_ent = cx.new(|cx| {
                    crate::mongodb::document_editor::DocumentEditorPanel::new_insert(
                        coll, window, cx,
                    )
                });
                register_dock_panel!(self, tab_spec_for_manager, panel_ent, window, cx);
            }
            TabSpec::ReleaseNotes { version } => {
                let tab_spec_for_manager = TabSpec::ReleaseNotes {
                    version: version.clone(),
                };
                let panel_ent =
                    cx.new(|cx| super::release_notes::ReleaseNotesPanel::new(version, window, cx));
                register_dock_panel!(self, tab_spec_for_manager, panel_ent, window, cx);
            }
            TabSpec::Welcome | TabSpec::Builtin { .. } => {}
        }
    }

    fn dock_add_and_register_tab(
        &mut self,
        spec: TabSpec,
        panel: Arc<dyn PanelView>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.dock_area.update(cx, |dock, ecx| {
            dock.add_panel(panel.clone(), DockPlacement::Center, None, window, ecx);
        });
        let view: AnyView = panel.as_ref().into();
        self.tab_manager.update(cx, |tm, ecx| {
            tm.open_or_focus(spec, view, ecx);
        });
        cx.notify();
    }
}
