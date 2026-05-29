use std::time::Instant;

use gpui::{App, Context, Entity, Window};

use crate::connection::{
    ConnectionEntry, ConnectionState, EngineKind, OpenedConnection, open_connection,
    opened_into_any,
};

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
        let same = self.selected_connection == Some(idx);
        self.selected_connection = Some(idx);
        self.selected_object = None;
        let conn_ent = match self.registry.read(cx).connections().get(idx) {
            Some(e) => e.clone(),
            None => return,
        };

        match conn_ent.read(cx).state {
            ConnectionState::Connecting { .. } => return,
            ConnectionState::Connected(_) => {
                if !same {
                    self.set_connection_expanded(idx, true, cx);
                    self.pending_open_connection = Some(idx);
                }
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
        self.bump_object_list_epoch(cx);
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
        let task = open_connection(config, cx);

        cx.spawn(async move |_, cx| {
            let result = task.await;
            cx.update(|app| {
                finish_connection_open(
                    result,
                    conn_ent,
                    tree,
                    idx_for_pending,
                    conn_label,
                    conn_engine,
                    app,
                );
            });
        })
        .detach();
    }
}

fn finish_connection_open(
    result: anyhow::Result<OpenedConnection>,
    conn_ent: Entity<ConnectionEntry>,
    tree: Entity<ConnectionTree>,
    idx_for_pending: usize,
    conn_label: String,
    conn_engine: EngineKind,
    app: &mut App,
) {
    let mut tray_fail: Option<(String, String, String)> = None;
    conn_ent.update(app, |entry, ecx| {
        match result {
            Ok(opened) => {
                entry.state = ConnectionState::Connected(opened_into_any(opened, ecx));
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
            tree.set_connection_expanded(idx_for_pending, true, ecx);
            tree.pending_open_connection = Some(idx_for_pending);
        }
        ecx.notify();
    });
}
