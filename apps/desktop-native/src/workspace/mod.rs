// workspace/ — Workspace entity, DockArea, sidebar, status bar, connection wiring.

pub mod item;
pub mod pane;
pub mod sidebar;
pub mod status_bar;
pub mod topbar;
pub mod welcome;

use std::sync::Arc;
use std::time::Instant;

use gpui::{
    Context, Entity, FocusHandle, Focusable, IntoElement, Render, Window, div, prelude::*,
};
use gpui_component::{
    ActiveTheme,
    StyledExt,
    dock::{DockArea, DockItem, PanelStyle},
    h_flex, v_flex,
};

use crate::connection::lifecycle::Connectable;
use crate::connection::registry::ConnectionRegistry;
use crate::connection::{
    AnyConnection, ConnectionConfig, ConnectionEntry, ConnectionState, EngineKind,
};
use crate::mongodb::{MongoConfig, MongoConnection};
use crate::postgres::{self, PostgresConfig, SslMode};
use crate::sqlite::{self, SqliteConfig, SqliteConnection};
use ::mongodb::bson::Document;

use status_bar::StatusBar;
use topbar::Topbar;
use welcome::WelcomePanel;

pub struct Workspace {
    registry: Entity<ConnectionRegistry>,
    dock_area: Entity<DockArea>,
    sidebar_collapsed: bool,
    focus_handle: FocusHandle,
    /// After async connect, open engine tabs on next render (needs `&mut Window`).
    pending_open_connection: Option<usize>,
}

impl Workspace {
    pub fn new(window: &mut Window, cx: &mut Context<Self>) -> Self {
        let registry = cx.new(ConnectionRegistry::new);
        registry.update(cx, |reg, cx| {
            for entry in Self::make_mock_connections() {
                reg.add(entry, cx);
            }
        });

        let dock_area = cx.new(|cx| {
            DockArea::new("workspace", Some(1), window, cx).panel_style(PanelStyle::TabBar)
        });

        let welcome = cx.new(|cx| WelcomePanel::new(window, cx));
        let center = DockItem::tab(welcome, &dock_area.downgrade(), window, cx);
        dock_area.update(cx, |area, cx| {
            area.set_center(center, window, cx);
        });

        let workspace = Self {
            registry: registry.clone(),
            dock_area,
            sidebar_collapsed: false,
            focus_handle: cx.focus_handle(),
            pending_open_connection: None,
        };

        let _ = cx.observe(&registry, |_ws, _reg, cx| {
            cx.notify();
        });

        workspace
    }

    fn make_mock_connections() -> Vec<ConnectionEntry> {
        let mut entries = Vec::new();

        entries.push(ConnectionEntry::new(ConnectionConfig::Postgres(
            PostgresConfig {
                label: "prod-postgres".to_string(),
                host: "localhost".to_string(),
                port: 5432,
                database: "prod".to_string(),
                username: "admin".to_string(),
                password: String::new(),
                ssl_mode: SslMode::Prefer,
            },
        )));

        entries.push(ConnectionEntry::new(ConnectionConfig::Postgres(
            PostgresConfig {
                label: "staging-postgres".to_string(),
                host: "staging.example.com".to_string(),
                port: 5432,
                database: "staging".to_string(),
                username: "app".to_string(),
                password: String::new(),
                ssl_mode: SslMode::Require,
            },
        )));

        let mut mongo = ConnectionEntry::new(ConnectionConfig::MongoDB(MongoConfig {
            label: "local-mongo".to_string(),
            uri: "mongodb://localhost:27017".to_string(),
            database: None,
            auth_source: None,
        }));
        mongo.state = ConnectionState::Failed {
            reason: "demo: click to retry".to_string(),
            attempted_at: Instant::now(),
        };
        entries.push(mongo);

        entries.push(ConnectionEntry::new(ConnectionConfig::SQLite(SqliteConfig {
            label: "app.db".to_string(),
            path: std::path::PathBuf::from("app.db"),
            wal: true,
        })));

        entries
    }

    fn on_connection_row_clicked(
        &mut self,
        idx: usize,
        _window: &mut Window,
        cx: &mut Context<Self>,
    ) {
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
        conn_ent.update(cx, |e, cx| {
            e.state = ConnectionState::Connecting {
                since: Instant::now(),
            };
            e.last_error = None;
            cx.notify();
        });
        cx.notify();

        let workspace = cx.entity().clone();
        let idx_for_pending = idx;

        match config {
            ConnectionConfig::SQLite(cfg) => {
                let task = SqliteConnection::open(cfg, cx);
                cx.spawn(async move |_, cx| {
                    let result = task.await;
                    let _ = cx.update(|cx| {
                        conn_ent.update(cx, |entry, cx| {
                            match result {
                                Ok(conn) => {
                                    let ent = cx.new(|_| conn);
                                    entry.state =
                                        ConnectionState::Connected(AnyConnection::SQLite(ent));
                                }
                                Err(err) => {
                                    entry.state = ConnectionState::Failed {
                                        reason: err.to_string(),
                                        attempted_at: Instant::now(),
                                    };
                                    entry.last_error = Some(err.to_string());
                                }
                            }
                            cx.notify();
                        });
                        workspace.update(cx, |ws, cx| {
                            if matches!(conn_ent.read(cx).state, ConnectionState::Connected(_)) {
                                ws.pending_open_connection = Some(idx_for_pending);
                            }
                            cx.notify();
                        });
                    });
                })
                .detach();
            }
            ConnectionConfig::Postgres(cfg) => {
                let task = postgres::PgConnection::open(cfg, cx);
                cx.spawn(async move |_, cx| {
                    let result = task.await;
                    let _ = cx.update(|cx| {
                        conn_ent.update(cx, |entry, cx| {
                            match result {
                                Ok(conn) => {
                                    let ent = cx.new(|_| conn);
                                    entry.state =
                                        ConnectionState::Connected(AnyConnection::Postgres(ent));
                                }
                                Err(err) => {
                                    entry.state = ConnectionState::Failed {
                                        reason: err.to_string(),
                                        attempted_at: Instant::now(),
                                    };
                                    entry.last_error = Some(err.to_string());
                                }
                            }
                            cx.notify();
                        });
                        workspace.update(cx, |ws, cx| {
                            if matches!(conn_ent.read(cx).state, ConnectionState::Connected(_)) {
                                ws.pending_open_connection = Some(idx_for_pending);
                            }
                            cx.notify();
                        });
                    });
                })
                .detach();
            }
            ConnectionConfig::MongoDB(cfg) => {
                let task = MongoConnection::open(cfg, cx);
                cx.spawn(async move |_, cx| {
                    let result = task.await;
                    let _ = cx.update(|cx| {
                        conn_ent.update(cx, |entry, cx| {
                            match result {
                                Ok(conn) => {
                                    let ent = cx.new(|_| conn);
                                    entry.state =
                                        ConnectionState::Connected(AnyConnection::MongoDB(ent));
                                }
                                Err(err) => {
                                    entry.state = ConnectionState::Failed {
                                        reason: err.to_string(),
                                        attempted_at: Instant::now(),
                                    };
                                    entry.last_error = Some(err.to_string());
                                }
                            }
                            cx.notify();
                        });
                        workspace.update(cx, |ws, cx| {
                            if matches!(conn_ent.read(cx).state, ConnectionState::Connected(_)) {
                                ws.pending_open_connection = Some(idx_for_pending);
                            }
                            cx.notify();
                        });
                    });
                })
                .detach();
            }
        }
    }

    fn open_connected_tabs(
        &mut self,
        ac: &AnyConnection,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let weak = self.dock_area.downgrade();
        let center = match ac {
            AnyConnection::SQLite(ent) => {
                let pool = ent.read(cx).pool.clone();
                let tree = cx.new(|cx| sqlite::tree::SchemaTreePanel::new(pool.clone(), window, cx));
                let query =
                    cx.new(|cx| sqlite::query_editor::QueryEditorPanel::new(pool.clone(), window, cx));
                let pragma =
                    cx.new(|cx| sqlite::pragma_browser::PragmaBrowserPanel::new(pool.clone(), window, cx));
                DockItem::tabs(
                    vec![Arc::new(tree), Arc::new(query), Arc::new(pragma)],
                    &weak,
                    window,
                    cx,
                )
            }
            AnyConnection::Postgres(ent) => {
                let pool = ent.read(cx).pool.clone();
                let tree =
                    cx.new(|cx| postgres::tree::SchemaTreePanel::new(pool.clone(), window, cx));
                let query =
                    cx.new(|cx| postgres::query_editor::QueryEditorPanel::new(pool.clone(), window, cx));
                let monitor =
                    cx.new(|cx| postgres::live_monitor::LiveMonitorPanel::new(pool.clone(), window, cx));
                DockItem::tabs(
                    vec![Arc::new(tree), Arc::new(query), Arc::new(monitor)],
                    &weak,
                    window,
                    cx,
                )
            }
            AnyConnection::MongoDB(ent) => {
                let db = ent.read(cx).database().clone();
                let coll: ::mongodb::Collection<Document> = db.collection("based_explorer");
                let tree =
                    cx.new(|cx| crate::mongodb::tree::CollectionsTreePanel::new(db.clone(), window, cx));
                let builder =
                    cx.new(|cx| crate::mongodb::pipeline_builder::PipelineBuilderPanel::new(coll.clone(), window, cx));
                let stream =
                    cx.new(|cx| crate::mongodb::change_stream::ChangeStreamPanel::new(coll, window, cx));
                DockItem::tabs(
                    vec![Arc::new(tree), Arc::new(builder), Arc::new(stream)],
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
}

impl Focusable for Workspace {
    fn focus_handle(&self, _: &gpui::App) -> FocusHandle {
        self.focus_handle.clone()
    }
}

impl Render for Workspace {
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
                self.open_connected_tabs(&ac, window, cx);
            }
        }

        let conn_list: Vec<Entity<ConnectionEntry>> =
            self.registry.read(cx).connections().to_vec();
        let conn_count = conn_list.len();
        let border = cx.theme().sidebar_border;
        let sidebar_bg = cx.theme().sidebar;
        let muted = cx.theme().muted_foreground;
        let sfg = cx.theme().sidebar_foreground;

        let sidebar = v_flex()
            .w(gpui::px(220.0))
            .h_full()
            .flex_shrink_0()
            .border_r_1()
            .border_color(border)
            .bg(sidebar_bg)
            .child(
                div()
                    .px_3()
                    .py_2()
                    .text_xs()
                    .font_bold()
                    .text_color(muted)
                    .child("CONNECTIONS"),
            )
            .children(conn_list.into_iter().enumerate().map(|(idx, ent)| {
                let entry = ent.read(cx);
                let state_color = state_dot_color(&entry.state);
                let engine_label = entry.config.engine().short_label();
                let conn_label = entry.config.label().to_string();
                let state_label = entry.state.label();
                let badge_color = engine_badge_color(entry.config.engine());

                h_flex()
                    .id(("conn-row", idx))
                    .px_3()
                    .py_2()
                    .gap_2()
                    .items_center()
                    .cursor_pointer()
                    .rounded_md()
                    .mx_1()
                    .hover(|s| s.bg(gpui::hsla(0.0, 0.0, 0.5, 0.08)))
                    .on_click(cx.listener(move |ws, _, window, cx| {
                        ws.on_connection_row_clicked(idx, window, cx);
                    }))
                    .child(
                        div()
                            .w_2()
                            .h_2()
                            .rounded_full()
                            .flex_shrink_0()
                            .bg(state_color),
                    )
                    .child(
                        div()
                            .text_xs()
                            .px_1()
                            .rounded_sm()
                            .bg(badge_color)
                            .text_color(cx.theme().foreground)
                            .child(engine_label),
                    )
                    .child(
                        div()
                            .flex_1()
                            .text_sm()
                            .text_color(sfg)
                            .truncate()
                            .child(conn_label),
                    )
                    .child(
                        div()
                            .text_xs()
                            .text_color(muted)
                            .child(state_label),
                    )
            }));

        v_flex()
            .size_full()
            .bg(cx.theme().background)
            .child(Topbar::new("No Project"))
            .child(
                h_flex()
                    .flex_1()
                    .overflow_hidden()
                    .child(sidebar)
                    .child(
                        div()
                            .flex_1()
                            .size_full()
                            .overflow_hidden()
                            .child(self.dock_area.clone()),
                    ),
            )
            .child(StatusBar::new(conn_count))
    }
}

fn state_dot_color(state: &ConnectionState) -> gpui::Hsla {
    match state {
        ConnectionState::Disconnected => gpui::hsla(0.0, 0.0, 0.5, 1.0),
        ConnectionState::Connecting { .. } => gpui::hsla(0.13, 0.95, 0.55, 1.0),
        ConnectionState::Connected(_) => gpui::hsla(0.35, 0.75, 0.45, 1.0),
        ConnectionState::Failed { .. } => gpui::hsla(0.0, 0.75, 0.5, 1.0),
    }
}

fn engine_badge_color(engine: EngineKind) -> gpui::Hsla {
    match engine {
        EngineKind::Postgres => gpui::hsla(0.58, 0.6, 0.35, 0.3),
        EngineKind::MongoDB => gpui::hsla(0.28, 0.6, 0.35, 0.3),
        EngineKind::SQLite => gpui::hsla(0.08, 0.5, 0.4, 0.3),
    }
}
