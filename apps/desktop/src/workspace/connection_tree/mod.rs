// Sidebar connection list + object browser (extracted from workspace).

use std::collections::HashMap;
use std::sync::Arc;
use std::time::Instant;

use crate::widgets::ui::SIDEBAR_INSET;
use gpui::{Context, Entity, EventEmitter, IntoElement, Render, Window, div, prelude::*, px};
use gpui_component::{
    ActiveTheme,
    dock::{DockArea, DockItem},
    list::ListState,
    v_flex,
};
use mongodb::Database;
use sqlx::{PgPool, Row, SqlitePool};

use crate::connection::lifecycle::Connectable;
use crate::connection::registry::{ConnectionRegistry, RegistryEvent};
use crate::connection::{
    AnyConnection, ConnectionConfig, ConnectionEntry, ConnectionId, ConnectionState, EngineKind,
};
use crate::mongodb::MongoConnection;
use crate::postgres;
use crate::sqlite::{self, SqliteConnection};
use ::mongodb::bson::Document;

use super::notify;
use super::tab_spec::TabSpec;

mod connection_browser;
mod connection_list;
mod object_list;
mod types;

pub use types::{ObjectKind, SchemaObject, TreeEvent};

use types::{ActiveObjects, ConnState};

use connection_list::ConnectionListDelegate;
use object_list::ObjectListDelegate;

pub struct ConnectionTree {
    pub registry: Entity<ConnectionRegistry>,
    dock_area: Entity<DockArea>,
    conn_states: HashMap<crate::connection::ConnectionId, ConnState>,
    #[allow(dead_code)]
    active_spec: Option<TabSpec>,
    pub(crate) selected_connection: Option<usize>,
    pub(crate) active_objects: ActiveObjects,
    pub(crate) selected_object: Option<String>,
    pub(crate) connection_list: Option<Entity<ListState<ConnectionListDelegate>>>,
    pub(crate) object_list: Option<Entity<ListState<ObjectListDelegate>>>,
    object_list_epoch: u64,
    object_list_last_synced: u64,
    pending_open_connection: Option<usize>,
}

impl ConnectionTree {
    pub fn new(
        registry: Entity<ConnectionRegistry>,
        dock_area: Entity<DockArea>,
        cx: &mut Context<Self>,
    ) -> Self {
        cx.subscribe(&registry, |this, _, event, cx| match event {
            RegistryEvent::Added(id) => {
                this.conn_states.entry(id.clone()).or_insert(ConnState {
                    expanded: false,
                    objects: None,
                    loading: false,
                });
                cx.notify();
            }
            RegistryEvent::Removed(id) => {
                this.conn_states.remove(id);
                this.selected_connection = None;
                this.active_objects = ActiveObjects::Empty;
                this.selected_object = None;
                this.bump_object_list_epoch();
                this.pending_open_connection = None;
                cx.notify();
            }
            RegistryEvent::StateChanged(_) => cx.notify(),
        })
        .detach();

        let mut conn_states = HashMap::new();
        for ent in registry.read(cx).connections().iter() {
            conn_states.insert(
                ent.read(cx).id.clone(),
                ConnState {
                    expanded: false,
                    objects: None,
                    loading: false,
                },
            );
        }

        Self {
            registry,
            dock_area,
            conn_states,
            active_spec: None,
            selected_connection: None,
            active_objects: ActiveObjects::Empty,
            selected_object: None,
            connection_list: None,
            object_list: None,
            object_list_epoch: 0,
            object_list_last_synced: u64::MAX,
            pending_open_connection: None,
        }
    }

    pub(crate) fn bump_object_list_epoch(&mut self) {
        self.object_list_epoch = self.object_list_epoch.wrapping_add(1);
    }

    pub(crate) fn open_new_query(&mut self, idx: usize, cx: &mut Context<Self>) {
        let Some(ent) = self.registry.read(cx).connections().get(idx) else {
            return;
        };
        let conn_id = ent.read(cx).id.clone();
        cx.emit(TreeEvent::OpenTab(TabSpec::blank_query_editor(conn_id)));
    }

    pub(crate) fn disconnect_at(&mut self, idx: usize, cx: &mut Context<Self>) {
        let ent = self.registry.read(cx).connections().get(idx).cloned();
        let Some(ent) = ent else {
            return;
        };
        ent.update(cx, |e, cx| {
            if let ConnectionState::Connected(ac) =
                std::mem::replace(&mut e.state, ConnectionState::Disconnected)
            {
                crate::connection::close_any_connection(ac, cx);
            }
            e.last_error = None;
            cx.notify();
        });
        cx.notify();
    }

    pub fn selected_connection_entry(&self, cx: &gpui::App) -> Option<Entity<ConnectionEntry>> {
        self.selected_connection
            .and_then(|idx| self.registry.read(cx).connections().get(idx).cloned())
    }

    /// Select a connection in the sidebar and, if connected, open its dashboard workspace (same as clicking the row).
    pub fn focus_connection_by_id(&mut self, conn_id: &ConnectionId, cx: &mut Context<Self>) {
        let Some(idx) = self
            .registry
            .read(cx)
            .connections()
            .iter()
            .position(|e| e.read(cx).id == *conn_id)
        else {
            return;
        };
        self.selected_connection = Some(idx);
        self.selected_object = None;
        self.bump_object_list_epoch();
        let conn_ent = self.registry.read(cx).connections()[idx].clone();
        if matches!(conn_ent.read(cx).state, ConnectionState::Connected(_)) {
            self.pending_open_connection = Some(idx);
        }
        cx.notify();
    }

    pub(crate) fn on_connection_row_clicked(
        &mut self,
        idx: usize,
        _window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.selected_connection = Some(idx);
        self.selected_object = None;
        self.bump_object_list_epoch();
        let conn_ent = match self.registry.read(cx).connections().get(idx) {
            Some(e) => e.clone(),
            None => return,
        };

        match conn_ent.read(cx).state {
            ConnectionState::Connecting { .. } => return,
            ConnectionState::Connected(_) => {
                self.pending_open_connection = Some(idx);
                cx.notify();
                return;
            }
            ConnectionState::Disconnected | ConnectionState::Failed { .. } => {}
        }

        let config = conn_ent.read(cx).config.clone();
        self.active_objects = ActiveObjects::Loading {
            label: config.label().to_string(),
            engine: config.engine(),
        };
        self.bump_object_list_epoch();
        conn_ent.update(cx, |e, cx| {
            e.state = ConnectionState::Connecting {
                since: Instant::now(),
            };
            e.last_error = None;
            cx.notify();
        });
        cx.notify();

        let tree = cx.entity().clone();
        let idx_for_pending = idx;
        let conn_label = config.label().to_string();
        let conn_engine = config.engine();

        match config {
            ConnectionConfig::SQLite(cfg) => {
                let task = SqliteConnection::open(cfg, cx);
                cx.spawn(async move |_, cx| {
                    let result = task.await;
                    cx.update(|app| {
                        let mut tray_fail: Option<(String, String, String)> = None;
                        conn_ent.update(app, |entry, ecx| {
                            match result {
                                Ok(conn) => {
                                    let ent = ecx.new(|_| conn);
                                    entry.state =
                                        ConnectionState::Connected(AnyConnection::SQLite(ent));
                                }
                                Err(err) => {
                                    log::warn!(
                                        "connection failed: label=\"{}\" engine={} error={:#}",
                                        conn_label,
                                        conn_engine.short_label(),
                                        err
                                    );
                                    entry.state = ConnectionState::Failed {
                                        reason: err.to_string(),
                                        attempted_at: Instant::now(),
                                    };
                                    entry.last_error = Some(err.to_string());
                                    tray_fail = Some((
                                        conn_label.clone(),
                                        conn_engine.short_label().to_string(),
                                        format!("{err:#}"),
                                    ));
                                }
                            }
                            ecx.notify();
                        });
                        if let Some((l, e, d)) = tray_fail {
                            notify::push_connection_failure(app, l, e, d);
                        }
                        tree.update(app, |tree, ecx| {
                            if matches!(conn_ent.read(ecx).state, ConnectionState::Connected(_)) {
                                tree.pending_open_connection = Some(idx_for_pending);
                            }
                            ecx.notify();
                        });
                    });
                })
                .detach();
            }
            ConnectionConfig::Postgres(cfg) => {
                let task = postgres::PgConnection::open(cfg, cx);
                cx.spawn(async move |_, cx| {
                    let result = task.await;
                    cx.update(|app| {
                        let mut tray_fail: Option<(String, String, String)> = None;
                        conn_ent.update(app, |entry, ecx| {
                            match result {
                                Ok(conn) => {
                                    let ent = ecx.new(|_| conn);
                                    entry.state =
                                        ConnectionState::Connected(AnyConnection::Postgres(ent));
                                }
                                Err(err) => {
                                    log::warn!(
                                        "connection failed: label=\"{}\" engine={} error={:#}",
                                        conn_label,
                                        conn_engine.short_label(),
                                        err
                                    );
                                    entry.state = ConnectionState::Failed {
                                        reason: err.to_string(),
                                        attempted_at: Instant::now(),
                                    };
                                    entry.last_error = Some(err.to_string());
                                    tray_fail = Some((
                                        conn_label.clone(),
                                        conn_engine.short_label().to_string(),
                                        format!("{err:#}"),
                                    ));
                                }
                            }
                            ecx.notify();
                        });
                        if let Some((l, e, d)) = tray_fail {
                            notify::push_connection_failure(app, l, e, d);
                        }
                        tree.update(app, |tree, ecx| {
                            if matches!(conn_ent.read(ecx).state, ConnectionState::Connected(_)) {
                                tree.pending_open_connection = Some(idx_for_pending);
                            }
                            ecx.notify();
                        });
                    });
                })
                .detach();
            }
            ConnectionConfig::MongoDB(cfg) => {
                let task = MongoConnection::open(cfg, cx);
                cx.spawn(async move |_, cx| {
                    let result = task.await;
                    cx.update(|app| {
                        let mut tray_fail: Option<(String, String, String)> = None;
                        conn_ent.update(app, |entry, ecx| {
                            match result {
                                Ok(conn) => {
                                    let ent = ecx.new(|_| conn);
                                    entry.state =
                                        ConnectionState::Connected(AnyConnection::MongoDB(ent));
                                }
                                Err(err) => {
                                    log::warn!(
                                        "connection failed: label=\"{}\" engine={} error={:#}",
                                        conn_label,
                                        conn_engine.short_label(),
                                        err
                                    );
                                    entry.state = ConnectionState::Failed {
                                        reason: err.to_string(),
                                        attempted_at: Instant::now(),
                                    };
                                    entry.last_error = Some(err.to_string());
                                    tray_fail = Some((
                                        conn_label.clone(),
                                        conn_engine.short_label().to_string(),
                                        format!("{err:#}"),
                                    ));
                                }
                            }
                            ecx.notify();
                        });
                        if let Some((l, e, d)) = tray_fail {
                            notify::push_connection_failure(app, l, e, d);
                        }
                        tree.update(app, |tree, ecx| {
                            if matches!(conn_ent.read(ecx).state, ConnectionState::Connected(_)) {
                                tree.pending_open_connection = Some(idx_for_pending);
                            }
                            ecx.notify();
                        });
                    });
                })
                .detach();
            }
        }
    }

    fn open_connected_workspace(
        &mut self,
        idx: usize,
        ac: &AnyConnection,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let Some(conn_ent) = self.registry.read(cx).connections().get(idx).cloned() else {
            return;
        };
        let (label, engine, conn_id) = {
            let entry = conn_ent.read(cx);
            (
                entry.config.label().to_string(),
                entry.config.engine(),
                entry.id.clone(),
            )
        };

        self.selected_connection = Some(idx);
        self.selected_object = None;
        self.active_objects = ActiveObjects::Loading {
            label: label.clone(),
            engine,
        };
        self.bump_object_list_epoch();
        self.load_objects_for_connection(idx, ac.clone(), cx);

        let weak = self.dock_area.downgrade();
        let dashboard = cx.new(|cx| {
            super::object_info::ConnectionDashboardPanel::new(conn_ent.clone(), window, cx)
        });
        let center = match ac {
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
                DockItem::tabs(
                    vec![Arc::new(dashboard), Arc::new(query), Arc::new(pragma)],
                    &weak,
                    window,
                    cx,
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
                let monitor = cx.new(|cx| {
                    postgres::live_monitor::LiveMonitorPanel::new(pool.clone(), window, cx)
                });
                DockItem::tabs(
                    vec![Arc::new(dashboard), Arc::new(query), Arc::new(monitor)],
                    &weak,
                    window,
                    cx,
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
                DockItem::tabs(
                    vec![Arc::new(dashboard), Arc::new(builder), Arc::new(stream)],
                    &weak,
                    window,
                    cx,
                )
            }
        };

        self.dock_area.update(cx, |dock, cx| {
            dock.set_center(center, window, cx);
        });
    }

    fn load_objects_for_connection(
        &mut self,
        idx: usize,
        ac: AnyConnection,
        cx: &mut Context<Self>,
    ) {
        let Some(ent) = self.registry.read(cx).connections().get(idx).cloned() else {
            return;
        };
        let entry = ent.read(cx);
        let label = entry.config.label().to_string();
        let engine = entry.config.engine();
        let _ = entry;

        match ac {
            AnyConnection::SQLite(conn) => {
                let pool = conn.read(cx).pool.clone();
                self.load_sqlite_objects(idx, label, engine, pool, cx);
            }
            AnyConnection::Postgres(conn) => {
                let pool = conn.read(cx).pool.clone();
                self.load_postgres_objects(idx, label, engine, pool, cx);
            }
            AnyConnection::MongoDB(conn) => {
                let db = conn.read(cx).database().clone();
                self.load_mongo_objects(idx, label, engine, db, cx);
            }
        }
    }

    fn load_sqlite_objects(
        &mut self,
        idx: usize,
        label: String,
        engine: EngineKind,
        pool: SqlitePool,
        cx: &mut Context<Self>,
    ) {
        self.active_objects = ActiveObjects::Loading {
            label: label.clone(),
            engine,
        };
        self.bump_object_list_epoch();
        cx.spawn(async move |this, cx| {
            let result = crate::db::run(cx, async move {
                let rows = sqlx::query(
                    "SELECT name, type FROM sqlite_master \
                     WHERE type IN ('table','view','trigger') \
                     ORDER BY type, name",
                )
                .fetch_all(&pool)
                .await?;

                let objects = rows
                    .iter()
                    .map(|row| {
                        let name: String = row.get("name");
                        let kind_str: String = row.get("type");
                        let kind = match kind_str.as_str() {
                            "view" => ObjectKind::View,
                            "trigger" => ObjectKind::Trigger,
                            _ => ObjectKind::Table,
                        };
                        SchemaObject {
                            name,
                            schema: None,
                            kind,
                        }
                    })
                    .collect::<Vec<_>>();
                Ok(objects)
            })
            .await;

            let _ = this.update(cx, |tree, cx| {
                if tree.selected_connection != Some(idx) {
                    return;
                }
                tree.active_objects = match result {
                    Ok(objects) => ActiveObjects::Ready {
                        label,
                        engine,
                        objects,
                    },
                    Err(err) => ActiveObjects::Error {
                        label,
                        message: err.to_string(),
                    },
                };
                tree.bump_object_list_epoch();
                cx.notify();
            });
        })
        .detach();
    }

    fn load_postgres_objects(
        &mut self,
        idx: usize,
        label: String,
        engine: EngineKind,
        pool: PgPool,
        cx: &mut Context<Self>,
    ) {
        self.active_objects = ActiveObjects::Loading {
            label: label.clone(),
            engine,
        };
        self.bump_object_list_epoch();
        cx.spawn(async move |this, cx| {
            let result = crate::db::run(cx, async move {
                let rows = sqlx::query(
                    r"SELECT table_schema, table_name, table_type
                      FROM information_schema.tables
                      WHERE table_schema NOT IN ('pg_catalog', 'information_schema')
                      ORDER BY table_schema, table_type, table_name",
                )
                .fetch_all(&pool)
                .await?;

                let objects = rows
                    .iter()
                    .map(|row| {
                        let schema: String = row.get("table_schema");
                        let name: String = row.get("table_name");
                        let ty: String = row.get("table_type");
                        let kind = match ty.as_str() {
                            "VIEW" => ObjectKind::View,
                            "MATERIALIZED VIEW" => ObjectKind::MaterializedView,
                            _ => ObjectKind::Table,
                        };
                        SchemaObject {
                            name,
                            schema: Some(schema),
                            kind,
                        }
                    })
                    .collect::<Vec<_>>();
                Ok(objects)
            })
            .await;

            let _ = this.update(cx, |tree, cx| {
                if tree.selected_connection != Some(idx) {
                    return;
                }
                tree.active_objects = match result {
                    Ok(objects) => ActiveObjects::Ready {
                        label,
                        engine,
                        objects,
                    },
                    Err(err) => ActiveObjects::Error {
                        label,
                        message: err.to_string(),
                    },
                };
                tree.bump_object_list_epoch();
                cx.notify();
            });
        })
        .detach();
    }

    fn load_mongo_objects(
        &mut self,
        idx: usize,
        label: String,
        engine: EngineKind,
        db: Database,
        cx: &mut Context<Self>,
    ) {
        self.active_objects = ActiveObjects::Loading {
            label: label.clone(),
            engine,
        };
        self.bump_object_list_epoch();
        cx.spawn(async move |this, cx| {
            let result = crate::db::run(cx, async move {
                let names = db.list_collection_names(None).await?;
                let objects = names
                    .into_iter()
                    .map(|name| SchemaObject {
                        name,
                        schema: None,
                        kind: ObjectKind::Collection,
                    })
                    .collect::<Vec<_>>();
                Ok(objects)
            })
            .await;

            let _ = this.update(cx, |tree, cx| {
                if tree.selected_connection != Some(idx) {
                    return;
                }
                tree.active_objects = match result {
                    Ok(objects) => ActiveObjects::Ready {
                        label,
                        engine,
                        objects,
                    },
                    Err(err) => ActiveObjects::Error {
                        label,
                        message: err.to_string(),
                    },
                };
                tree.bump_object_list_epoch();
                cx.notify();
            });
        })
        .detach();
    }

    pub(crate) fn on_object_clicked(
        &mut self,
        object: SchemaObject,
        _window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.selected_object = Some(object.display_name());
        let Some(idx) = self.selected_connection else {
            return;
        };
        let Some(ent) = self.registry.read(cx).connections().get(idx).cloned() else {
            return;
        };
        let ac = match &ent.read(cx).state {
            ConnectionState::Connected(ac) => Some(ac.clone()),
            _ => None,
        };
        match ac {
            Some(AnyConnection::SQLite(_)) => match object.kind {
                ObjectKind::Table | ObjectKind::View => {
                    cx.emit(TreeEvent::OpenTab(TabSpec::DataViewer {
                        conn_id: ent.read(cx).id.clone(),
                        object: object.display_name(),
                    }));
                }
                _ => self.emit_object_info_tab(object, cx),
            },
            Some(AnyConnection::Postgres(_)) => match object.kind {
                ObjectKind::Table | ObjectKind::View | ObjectKind::MaterializedView => {
                    cx.emit(TreeEvent::OpenTab(TabSpec::DataViewer {
                        conn_id: ent.read(cx).id.clone(),
                        object: object.display_name(),
                    }));
                }
                _ => self.emit_object_info_tab(object, cx),
            },
            Some(AnyConnection::MongoDB(_)) => {
                if matches!(object.kind, ObjectKind::Collection) {
                    cx.emit(TreeEvent::OpenTab(TabSpec::DataViewer {
                        conn_id: ent.read(cx).id.clone(),
                        object: object.display_name(),
                    }));
                } else {
                    self.emit_object_info_tab(object, cx);
                }
            }
            _ => {}
        }
        cx.notify();
    }

    fn emit_object_info_tab(&mut self, object: SchemaObject, cx: &mut Context<Self>) {
        let Some(idx) = self.selected_connection else {
            return;
        };
        let Some(ent) = self.registry.read(cx).connections().get(idx).cloned() else {
            return;
        };
        let conn_id = ent.read(cx).id.clone();
        cx.emit(TreeEvent::OpenTab(TabSpec::ObjectInfo {
            conn_id,
            object_name: object.display_name(),
            kind_label: object.kind.label().to_string(),
        }));
    }

    pub(crate) fn open_inspector_tab(
        &mut self,
        object: SchemaObject,
        conn_id: ConnectionId,
        cx: &mut Context<Self>,
    ) {
        cx.emit(TreeEvent::OpenTab(TabSpec::Inspector {
            conn_id,
            object: object.display_name(),
        }));
    }

    pub(crate) fn open_document_insert_tab(
        &mut self,
        object: SchemaObject,
        conn_id: ConnectionId,
        cx: &mut Context<Self>,
    ) {
        cx.emit(TreeEvent::OpenTab(TabSpec::DocumentInsert {
            conn_id,
            collection: object.display_name(),
        }));
    }
}

impl Render for ConnectionTree {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        if let Some(idx) = self.pending_open_connection.take() {
            let ac = if let Some(ent) = self.registry.read(cx).connections().get(idx) {
                match &ent.read(cx).state {
                    ConnectionState::Connected(ac) => Some(ac.clone()),
                    _ => None,
                }
            } else {
                None
            };
            if let Some(ac) = ac {
                self.open_connected_workspace(idx, &ac, window, cx);
            }
        }

        let connection_list = connection_list::ensure_connection_list(self, window, cx);
        connection_list::refresh_connection_list(self, cx);
        let object_list = object_list::ensure_object_list(self, window, cx);
        object_list::refresh_object_list(self, cx);
        let active_objects = self.active_objects.clone();

        v_flex()
            .size_full()
            .min_h_0()
            .child(connection_browser::render_connections_pane(
                connection_list,
                cx,
            ))
            .child(connection_browser::render_objects_pane(
                active_objects,
                object_list,
                cx,
            ))
    }
}

impl EventEmitter<TreeEvent> for ConnectionTree {}

impl ConnectionTree {
    /// Connected connections' cached schema objects matching `query` (palette search).
    pub fn schema_palette_matches(
        &self,
        query: &str,
        cx: &gpui::App,
    ) -> Vec<(ConnectionId, SchemaObject, EngineKind)> {
        let q = query.to_lowercase();
        let mut out = Vec::new();
        for (conn_id, state) in &self.conn_states {
            let Some(entry) = self.registry.read(cx).get(conn_id, cx) else {
                continue;
            };
            let entry = entry.read(cx);
            if !matches!(entry.state, ConnectionState::Connected(_)) {
                continue;
            }
            let engine = entry.config.engine();
            let Some(objects) = state.objects.as_ref() else {
                continue;
            };
            for obj in objects {
                let name = obj.display_name();
                if q.is_empty()
                    || name.to_lowercase().contains(&q)
                    || conn_id.0.to_lowercase().contains(&q)
                {
                    out.push((conn_id.clone(), obj.clone(), engine));
                }
            }
        }
        out
    }
}
