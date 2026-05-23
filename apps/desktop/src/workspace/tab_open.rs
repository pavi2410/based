//! Cross-panel tab open requests (query editor → workspace).

use gpui::{App, BorrowAppContext, Entity, Global};

use crate::connection::ConnectionId;

use super::{TabManager, TabSpec};

#[derive(Clone)]
pub struct TabManagerRef(pub Entity<TabManager>);

impl Global for TabManagerRef {}

/// Mark the active query tab for this connection as having unsaved edits.
pub fn mark_query_tab_dirty(conn_id: &ConnectionId, cx: &mut App) {
    let Some(handle) = cx.try_global::<TabManagerRef>().map(|h| h.0.clone()) else {
        return;
    };
    handle.update(cx, |tm, cx| {
        let Some(active) = tm.active_idx else {
            return;
        };
        let Some(tab) = tm.tabs.get_mut(active) else {
            return;
        };
        if tab.spec.conn_id() == conn_id && matches!(tab.spec, TabSpec::QueryEditor { .. }) {
            tab.dirty = true;
            cx.notify();
        }
    });
}

#[derive(Default)]
pub struct TabOpenQueue {
    pub pending: Option<TabSpec>,
}

impl Global for TabOpenQueue {}

/// Inject SQL into the active query editor for `conn_id` (command palette history).
#[derive(Default)]
pub struct SqlInject {
    pub target: Option<(crate::connection::ConnectionId, String)>,
}

impl Global for SqlInject {}

pub fn enqueue_sql_inject(
    conn_id: crate::connection::ConnectionId,
    sql: String,
    cx: &mut impl BorrowAppContext,
) {
    cx.update_global(|inj: &mut SqlInject, _| {
        inj.target = Some((conn_id, sql));
    });
}

pub fn take_sql_inject(
    conn_id: &crate::connection::ConnectionId,
    cx: &mut impl BorrowAppContext,
) -> Option<String> {
    cx.update_global(|inj: &mut SqlInject, _| {
        if inj.target.as_ref().is_some_and(|(c, _)| c == conn_id) {
            inj.target.take().map(|(_, sql)| sql)
        } else {
            None
        }
    })
}

pub fn enqueue_open_tab(spec: TabSpec, cx: &mut impl BorrowAppContext) {
    cx.update_global(|q: &mut TabOpenQueue, _| {
        q.pending = Some(spec);
    });
}
