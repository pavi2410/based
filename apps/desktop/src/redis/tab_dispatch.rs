//! Panel construction for Redis tabs.

use std::sync::Arc;

use based_redis::Conn;
use gpui_kit::component::dock::PanelView;
use gpui_kit::{Context, Window, prelude::*};

use crate::connection::ConnectionId;
use crate::workspace::Workspace;
use crate::workspace::tabs::label::tab_label_for_spec;
use crate::workspace::tabs::spec::{QueryEditorInit, TabSpec};

use super::console::ConsolePanel;
use super::key_view::KeyViewPanel;

/// Try to build a Redis panel for `spec`. Returns `None` for tab kinds Redis doesn't handle.
pub fn build_panel(
    spec: &TabSpec,
    conn: Conn,
    conn_id: &ConnectionId,
    window: &mut Window,
    cx: &mut Context<Workspace>,
) -> Option<Arc<dyn PanelView>> {
    match spec {
        TabSpec::DataViewer { object, .. } => {
            let key = object.clone();
            let panel = cx.new(|cx| KeyViewPanel::new(conn, key, window, cx));
            panel.update(cx, |p, _| p.tab_label = tab_label_for_spec(spec, false));
            Some(Arc::new(panel))
        }
        TabSpec::QueryEditor { init, .. } => {
            let initial = match init {
                QueryEditorInit::Sql { sql, .. } => sql.clone(),
                QueryEditorInit::MongoPipeline { .. } => None,
            };
            let panel = cx.new(|cx| ConsolePanel::new(conn, conn_id.clone(), initial, window, cx));
            Some(Arc::new(panel))
        }
        _ => None,
    }
}
