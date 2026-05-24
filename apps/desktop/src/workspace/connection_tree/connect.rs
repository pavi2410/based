use std::time::Instant;

use gpui::{Context, Window, prelude::*};

use crate::connection::lifecycle::Connectable;
use crate::connection::{AnyConnection, ConnectionConfig, ConnectionState};
use crate::mongodb::MongoConnection;
use crate::postgres;
use crate::sqlite::SqliteConnection;

use super::ConnectionTree;
use super::notify;
use super::types::ActiveObjects;

impl ConnectionTree {
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
}
