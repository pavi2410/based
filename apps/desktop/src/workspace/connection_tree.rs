// Sidebar connection list + object browser (extracted from workspace).

use std::collections::HashMap;
use std::collections::hash_map::DefaultHasher;
use std::hash::{Hash, Hasher};
use std::sync::Arc;
use std::time::Instant;

use crate::widgets::ui::{engine_chip, engine_color};
use gpui::{
    AnyElement, Context, Entity, EventEmitter, FontWeight, IntoElement, Render, SharedString,
    Window, div, prelude::*,
};
use gpui_component::{
    ActiveTheme, Icon, IconName, Sizable as _, StyledExt,
    button::{Button, ButtonVariants},
    dock::{DockArea, DockItem},
    h_flex,
    scroll::ScrollableElement,
    tooltip::Tooltip,
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

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ObjectKind {
    Table,
    View,
    MaterializedView,
    Trigger,
    Collection,
}

impl ObjectKind {
    fn label(&self) -> &'static str {
        match self {
            Self::Table => "table",
            Self::View => "view",
            Self::MaterializedView => "matview",
            Self::Trigger => "trigger",
            Self::Collection => "collection",
        }
    }

    pub fn group(&self) -> &'static str {
        match self {
            Self::Table => "Tables",
            Self::View | Self::MaterializedView => "Views",
            Self::Trigger => "Triggers",
            Self::Collection => "Collections",
        }
    }

    pub fn icon(&self) -> &'static str {
        match self {
            Self::Table => "▤",
            Self::View | Self::MaterializedView => "◈",
            Self::Trigger => "⚡",
            Self::Collection => "▦",
        }
    }
}

/// Schema browser row (PostgreSQL exposes `schema` + local name).
#[derive(Clone, Debug)]
pub struct SchemaObject {
    pub name: String,
    pub schema: Option<String>,
    pub kind: ObjectKind,
}

impl SchemaObject {
    pub fn display_name(&self) -> String {
        if let Some(schema) = &self.schema {
            format!("{schema}.{}", self.name)
        } else {
            self.name.clone()
        }
    }
}

#[derive(Clone, Debug)]
enum ActiveObjects {
    Empty,
    Loading {
        label: String,
        engine: EngineKind,
    },
    Ready {
        label: String,
        engine: EngineKind,
        objects: Vec<SchemaObject>,
    },
    Error {
        label: String,
        message: String,
    },
}

/// Reserved for per-connection expansion / cached schema.
struct ConnState {
    expanded: bool,
    objects: Option<Vec<SchemaObject>>,
    loading: bool,
}

#[derive(Clone)]
pub enum TreeEvent {
    OpenTab(TabSpec),
}

pub struct ConnectionTree {
    pub registry: Entity<ConnectionRegistry>,
    dock_area: Entity<DockArea>,
    conn_states: HashMap<crate::connection::ConnectionId, ConnState>,
    #[allow(dead_code)]
    active_spec: Option<TabSpec>,
    selected_connection: Option<usize>,
    active_objects: ActiveObjects,
    selected_object: Option<String>,
    pending_open_connection: Option<usize>,
}

impl ConnectionTree {
    pub fn new(
        registry: Entity<ConnectionRegistry>,
        dock_area: Entity<DockArea>,
        cx: &mut Context<Self>,
    ) -> Self {
        cx.subscribe(&registry, |this, _, event, cx| match event {
            RegistryEvent::ConnectionAdded(id) => {
                this.conn_states.entry(id.clone()).or_insert(ConnState {
                    expanded: false,
                    objects: None,
                    loading: false,
                });
                cx.notify();
            }
            RegistryEvent::ConnectionRemoved(id) => {
                this.conn_states.remove(id);
                this.selected_connection = None;
                this.active_objects = ActiveObjects::Empty;
                this.selected_object = None;
                this.pending_open_connection = None;
                cx.notify();
            }
            RegistryEvent::ConnectionStateChanged(_) => cx.notify(),
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
            pending_open_connection: None,
        }
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
        let conn_ent = self.registry.read(cx).connections()[idx].clone();
        if matches!(conn_ent.read(cx).state, ConnectionState::Connected(_)) {
            self.pending_open_connection = Some(idx);
        }
        cx.notify();
    }

    fn on_connection_row_clicked(
        &mut self,
        idx: usize,
        _window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.selected_connection = Some(idx);
        self.selected_object = None;
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
                    let _ = cx.update(|app| {
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
                    let _ = cx.update(|app| {
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
                    let _ = cx.update(|app| {
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
                cx.notify();
            });
        })
        .detach();
    }

    fn on_object_clicked(
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

    fn open_inspector_tab(&mut self, object: SchemaObject, conn_id: ConnectionId, cx: &mut Context<Self>) {
        cx.emit(TreeEvent::OpenTab(TabSpec::Inspector {
            conn_id,
            object: object.display_name(),
        }));
    }

    fn open_document_insert_tab(
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

        let conn_list: Vec<Entity<ConnectionEntry>> = self.registry.read(cx).connections().to_vec();
        let conn_count = conn_list.len();
        let connected_count = conn_list
            .iter()
            .filter(|ent| matches!(ent.read(cx).state, ConnectionState::Connected(_)))
            .count();
        let border = cx.theme().sidebar_border;
        let muted = cx.theme().muted_foreground;
        let sfg = cx.theme().sidebar_foreground;
        let list_hover = cx.theme().list_hover;

        let connections_pane = v_flex()
            .h(gpui::px(250.0))
            .min_h(gpui::px(170.0))
            .max_h(gpui::px(270.0))
            .flex_shrink_0()
            .border_b_1()
            .border_color(border)
            .child(
                h_flex()
                    .h(gpui::px(38.0))
                    .px_2()
                    .gap_2()
                    .items_center()
                    .border_b_1()
                    .border_color(border.opacity(0.86))
                    .child(
                        v_flex()
                            .flex_1()
                            .min_w_0()
                            .child(
                                div()
                                    .text_xs()
                                    .font_bold()
                                    .text_color(muted)
                                    .font_family(cx.theme().mono_font_family.clone())
                                    .child("CONNECTIONS"),
                            )
                            .child(
                                div()
                                    .text_xs()
                                    .text_color(muted.opacity(0.82))
                                    .truncate()
                                    .child(format!(
                                        "{conn_count} connections · {connected_count} live"
                                    )),
                            ),
                    )
                    .child(
                        div()
                            .h(gpui::px(22.0))
                            .px(gpui::px(7.0))
                            .rounded(gpui::px(5.0))
                            .border_1()
                            .border_color(border.opacity(0.8))
                            .bg(cx.theme().muted.opacity(0.38))
                            .text_xs()
                            .text_color(muted)
                            .flex()
                            .items_center()
                            .child("+"),
                    ),
            )
            .child(
                h_flex()
                    .mx_2()
                    .my_2()
                    .h(gpui::px(28.0))
                    .items_center()
                    .gap_2()
                    .px_2()
                    .rounded(gpui::px(6.0))
                    .border_1()
                    .border_color(border.opacity(0.78))
                    .bg(cx.theme().muted.opacity(0.32))
                    .child(
                        Icon::new(IconName::Search)
                            .with_size(gpui_component::Size::XSmall)
                            .text_color(muted),
                    )
                    .child(
                        div()
                            .text_xs()
                            .text_color(muted)
                            .truncate()
                            .child("Search connections"),
                    ),
            )
            .child(v_flex().flex_1().overflow_y_scrollbar().children(
                conn_list.into_iter().enumerate().map(|(idx, ent)| {
                    let entry = ent.read(cx);
                    let state_color = connection_state_dot(&entry.state, cx.theme());
                    let engine = entry.config.engine();
                    let conn_label = entry.config.label().to_string();
                    let state_label = entry.state.label();
                    let is_selected = self.selected_connection == Some(idx);
                    let is_failed = matches!(entry.state, ConnectionState::Failed { .. });
                    let fail_reason = match &entry.state {
                        ConnectionState::Failed { reason, .. } => Some(reason.clone()),
                        _ => None,
                    };
                    let err_fg = cx.theme().danger_foreground;

                    let status_cell = if is_failed {
                        h_flex()
                            .flex_shrink_0()
                            .gap_1()
                            .items_center()
                            .child(
                                Icon::new(IconName::CircleX)
                                    .text_color(err_fg)
                                    .with_size(gpui_component::Size::XSmall),
                            )
                            .child(
                                div()
                                    .text_xs()
                                    .font_semibold()
                                    .text_color(err_fg)
                                    .child("Failed"),
                            )
                    } else {
                        h_flex()
                            .flex_shrink_0()
                            .child(div().text_xs().text_color(muted).child(state_label))
                    };

                    let main_row = h_flex()
                        .w_full()
                        .gap_2()
                        .items_center()
                        .child(
                            div()
                                .w_2()
                                .h_2()
                                .rounded_full()
                                .flex_shrink_0()
                                .bg(state_color),
                        )
                        .child(
                            v_flex()
                                .flex_1()
                                .min_w_0()
                                .gap(gpui::px(1.0))
                                .child(
                                    div()
                                        .text_sm()
                                        .text_color(sfg)
                                        .truncate()
                                        .when(is_failed, |d| d.text_color(err_fg.opacity(0.92)))
                                        .child(conn_label.clone()),
                                )
                                .child(
                                    h_flex()
                                        .gap_1()
                                        .items_center()
                                        .child(engine_chip(engine, cx))
                                        .child(
                                            div()
                                                .text_xs()
                                                .text_color(muted.opacity(0.82))
                                                .child("local"),
                                        ),
                                ),
                        )
                        .child(status_cell);

                    let mut row = main_row.id(("conn-row", idx));
                    if let Some(reason) = fail_reason {
                        let reason_tip: SharedString = reason.clone().into();
                        row = row.tooltip(move |window, app| {
                            Tooltip::element({
                                let reason_tip = reason_tip.clone();
                                move |_w, tip_cx| {
                                    let fg = tip_cx.theme().foreground;
                                    let subtle = tip_cx.theme().muted_foreground;
                                    v_flex()
                                        .gap_1()
                                        .max_w(gpui::px(400.0))
                                        .child(
                                            div()
                                                .text_xs()
                                                .font_semibold()
                                                .text_color(fg)
                                                .child("Could not connect"),
                                        )
                                        .child(
                                            div()
                                                .text_xs()
                                                .text_color(subtle)
                                                .font_family(
                                                    tip_cx.theme().mono_font_family.clone(),
                                                )
                                                .child(reason_tip.clone()),
                                        )
                                }
                            })
                            .build(window, app)
                        });
                    }

                    row.px_2()
                        .py_1()
                        .cursor_pointer()
                        .rounded(gpui::px(7.0))
                        .mx_2()
                        .mb(gpui::px(2.0))
                        .when(is_selected, |r| {
                            r.bg(cx.theme().accent.opacity(0.22))
                                .border_1()
                                .border_color(engine_color(engine).opacity(0.26))
                        })
                        .when(is_failed, |r| {
                            r.border_1().border_color(cx.theme().danger.opacity(0.35))
                        })
                        .hover(move |s| s.bg(list_hover))
                        .on_click(cx.listener(move |tree, _, window, cx| {
                            tree.on_connection_row_clicked(idx, window, cx);
                        }))
                }),
            ));

        let conn_id_for_tabs = self.selected_connection.and_then(|idx| {
            self.registry
                .read(cx)
                .connections()
                .get(idx)
                .map(|e| e.read(cx).id.clone())
        });
        let objects_pane = render_objects_pane(
            self.active_objects.clone(),
            self.selected_object.clone(),
            conn_id_for_tabs,
            cx,
        );

        v_flex()
            .size_full()
            .child(connections_pane)
            .child(objects_pane)
    }
}

fn render_objects_pane(
    active_objects: ActiveObjects,
    selected_object: Option<String>,
    conn_id_for_tabs: Option<ConnectionId>,
    cx: &mut Context<ConnectionTree>,
) -> impl IntoElement {
    let border = cx.theme().sidebar_border;
    let muted = cx.theme().muted_foreground;

    let body: AnyElement = match active_objects {
        ActiveObjects::Empty => v_flex()
            .flex_1()
            .items_center()
            .justify_center()
            .p_3()
            .child(
                div()
                    .text_xs()
                    .text_color(muted)
                    .child("Select a connected database to browse objects."),
            )
            .into_any_element(),
        ActiveObjects::Loading { label, engine } => v_flex()
            .flex_1()
            .p_3()
            .gap_2()
            .child(
                h_flex()
                    .gap_2()
                    .items_center()
                    .child(engine_chip(engine, cx))
                    .child(div().text_xs().text_color(muted).truncate().child(label)),
            )
            .child(
                div()
                    .text_xs()
                    .text_color(muted)
                    .child("Loading objects..."),
            )
            .into_any_element(),
        ActiveObjects::Error { label, message } => v_flex()
            .flex_1()
            .p_3()
            .gap_2()
            .child(
                div()
                    .text_xs()
                    .font_weight(FontWeight::SEMIBOLD)
                    .text_color(cx.theme().danger_foreground)
                    .child(format!("Could not load {label}")),
            )
            .child(
                div()
                    .text_xs()
                    .text_color(muted)
                    .child(notify::error_one_liner(&message)),
            )
            .into_any_element(),
        ActiveObjects::Ready {
            label,
            engine,
            objects,
        } => {
            let mut groups: Vec<(&'static str, Vec<SchemaObject>)> = Vec::new();
            for object in objects {
                let group = object.kind.group();
                if let Some((_, rows)) = groups.iter_mut().find(|(name, _)| *name == group) {
                    rows.push(object);
                } else {
                    groups.push((group, vec![object]));
                }
            }

            v_flex()
                .flex_1()
                .min_h_0()
                .overflow_y_scrollbar()
                .child(
                    h_flex()
                        .px_2()
                        .py_2()
                        .gap_2()
                        .items_center()
                        .child(engine_chip(engine, cx))
                        .child(div().text_xs().text_color(muted).truncate().child(label)),
                )
                .children(groups.into_iter().map(|(group_name, rows)| {
                    v_flex()
                        .mb(gpui::px(4.0))
                        .child(
                            h_flex()
                                .px_2()
                                .py_1()
                                .items_center()
                                .justify_between()
                                .child(
                                    div()
                                        .text_xs()
                                        .font_bold()
                                        .font_family(cx.theme().mono_font_family.clone())
                                        .text_color(muted.opacity(0.86))
                                        .child(group_name),
                                )
                                .child(
                                    div()
                                        .text_xs()
                                        .font_family(cx.theme().mono_font_family.clone())
                                        .text_color(muted.opacity(0.76))
                                        .child(rows.len().to_string()),
                                ),
                        )
                        .children(rows.into_iter().map(|object| {
                            let object_id = object.display_name();
                            let mut hasher = DefaultHasher::new();
                            object_id.hash(&mut hasher);
                            object.kind.label().hash(&mut hasher);
                            let object_key = hasher.finish();
                            let is_selected =
                                selected_object.as_deref() == Some(object_id.as_str());
                            let kind = object.kind.label();
                            let object_for_row_click = object.clone();

                            h_flex()
                                .id(("object-row", object_key))
                                .mx_2()
                                .mb(gpui::px(1.0))
                                .px_2()
                                .py(gpui::px(5.0))
                                .gap_2()
                                .items_center()
                                .rounded(gpui::px(6.0))
                                .cursor_pointer()
                                .when(is_selected, |row| {
                                    row.bg(cx.theme().accent.opacity(0.18))
                                        .border_1()
                                        .border_color(cx.theme().accent.opacity(0.28))
                                })
                                .hover(|row| row.bg(cx.theme().list_hover))
                                .on_click(cx.listener(move |tree, _, window, cx| {
                                    tree.on_object_clicked(object_for_row_click.clone(), window, cx);
                                }))
                                .child(
                                    div()
                                        .w(gpui::px(30.0))
                                        .text_xs()
                                        .font_family(cx.theme().mono_font_family.clone())
                                        .text_color(muted)
                                        .child(kind),
                                )
                                .child(
                                    div()
                                        .flex_1()
                                        .min_w_0()
                                        .text_sm()
                                        .text_color(cx.theme().sidebar_foreground)
                                        .truncate()
                                        .child(object_id),
                                )
                                .when(
                                    conn_id_for_tabs.is_some()
                                        && matches!(
                                            object.kind,
                                            ObjectKind::Table
                                                | ObjectKind::View
                                                | ObjectKind::MaterializedView
                                                | ObjectKind::Collection
                                        ),
                                    |row| {
                                        let cid = conn_id_for_tabs.clone().unwrap();
                                        let o = object.clone();
                                        row.child(
                                            Button::new(("obj-inspect", object_key))
                                                .small()
                                                .ghost()
                                                .label("◇")
                                                .on_click(cx.listener(move |tree, _, _, cx| {
                                                    cx.stop_propagation();
                                                    tree.open_inspector_tab(o.clone(), cid.clone(), cx);
                                                })),
                                        )
                                    },
                                )
                                .when(
                                    conn_id_for_tabs.is_some()
                                        && matches!(engine, EngineKind::MongoDB)
                                        && matches!(object.kind, ObjectKind::Collection),
                                    |row| {
                                        let cid = conn_id_for_tabs.clone().unwrap();
                                        let o = object.clone();
                                        row.child(
                                            Button::new(("obj-insert", object_key))
                                                .small()
                                                .ghost()
                                                .label("+")
                                                .on_click(cx.listener(move |tree, _, _, cx| {
                                                    cx.stop_propagation();
                                                    tree.open_document_insert_tab(o.clone(), cid.clone(), cx);
                                                })),
                                        )
                                    },
                                )
                        }))
                }))
                .into_any_element()
        }
    };

    v_flex()
        .flex_1()
        .min_h_0()
        .child(
            h_flex()
                .h(gpui::px(38.0))
                .px_2()
                .items_center()
                .justify_between()
                .border_b_1()
                .border_color(border.opacity(0.86))
                .child(
                    div()
                        .text_xs()
                        .font_bold()
                        .text_color(muted)
                        .font_family(cx.theme().mono_font_family.clone())
                        .child("OBJECTS"),
                )
                .child(
                    div()
                        .text_xs()
                        .text_color(muted.opacity(0.78))
                        .child("connection scoped"),
                ),
        )
        .child(
            h_flex()
                .mx_2()
                .my_2()
                .h(gpui::px(28.0))
                .items_center()
                .gap_2()
                .px_2()
                .rounded(gpui::px(6.0))
                .border_1()
                .border_color(border.opacity(0.78))
                .bg(cx.theme().muted.opacity(0.32))
                .child(
                    Icon::new(IconName::Search)
                        .with_size(gpui_component::Size::XSmall)
                        .text_color(muted),
                )
                .child(
                    div()
                        .text_xs()
                        .text_color(muted)
                        .truncate()
                        .child("Search objects"),
                ),
        )
        .child(body)
}

impl EventEmitter<TreeEvent> for ConnectionTree {}

fn connection_state_dot(state: &ConnectionState, t: &gpui_component::Theme) -> gpui::Hsla {
    match state {
        ConnectionState::Disconnected => t.muted_foreground.opacity(0.75),
        ConnectionState::Connecting { .. } => t.yellow.opacity(0.95),
        ConnectionState::Connected(_) => t.green_light,
        ConnectionState::Failed { .. } => t.red,
    }
}
