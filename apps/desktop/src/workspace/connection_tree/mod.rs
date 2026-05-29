// Sidebar connection list + object browser (extracted from workspace).

use std::collections::HashMap;

use gpui::{Context, Entity, EventEmitter, IntoElement, Render, Window, prelude::*};
use gpui_component::{dock::DockArea, list::ListState, v_flex};

use crate::connection::registry::{ConnectionRegistry, RegistryEvent};
use crate::connection::{
    AnyConnection, ConnectionEntry, ConnectionId, ConnectionState, EngineKind,
};

use super::notify;
use super::tab_spec::TabSpec;

mod browser_list;
mod connect;
mod connection_browser;
mod connection_list;
mod object_list;
mod open_workspace;
mod schema_load;
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
    pub(crate) browser_list: Option<Entity<ListState<browser_list::BrowserListDelegate>>>,
    #[allow(dead_code)]
    pub(crate) connection_list: Option<Entity<ListState<ConnectionListDelegate>>>,
    #[allow(dead_code)]
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
                    error: None,
                });
                cx.notify();
            }
            RegistryEvent::Removed(id) => {
                this.conn_states.remove(id);
                this.selected_connection = None;
                this.active_objects = ActiveObjects::Empty;
                this.selected_object = None;
                this.bump_object_list_epoch(cx);
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
                    error: None,
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
            browser_list: None,
            connection_list: None,
            object_list: None,
            object_list_epoch: 0,
            object_list_last_synced: u64::MAX,
            pending_open_connection: None,
        }
    }

    pub(crate) fn bump_object_list_epoch(&mut self, cx: &mut Context<Self>) {
        self.object_list_epoch = self.object_list_epoch.wrapping_add(1);
        browser_list::refresh_browser_list(self, cx);
    }

    pub(crate) fn toggle_connection_expanded(&mut self, idx: usize, cx: &mut Context<Self>) {
        let Some(conn_id) = self
            .registry
            .read(cx)
            .connections()
            .get(idx)
            .map(|e| e.read(cx).id.clone())
        else {
            return;
        };
        if let Some(st) = self.conn_states.get_mut(&conn_id) {
            st.expanded = !st.expanded;
            if st.expanded {
                self.maybe_load_schema_for_connection(idx, cx);
            }
        }
        self.bump_object_list_epoch(cx);
    }

    pub(crate) fn set_connection_expanded(
        &mut self,
        idx: usize,
        expanded: bool,
        cx: &mut Context<Self>,
    ) {
        let Some(conn_id) = self
            .registry
            .read(cx)
            .connections()
            .get(idx)
            .map(|e| e.read(cx).id.clone())
        else {
            return;
        };
        if let Some(st) = self.conn_states.get_mut(&conn_id) {
            st.expanded = expanded;
            if expanded {
                self.maybe_load_schema_for_connection(idx, cx);
            }
        }
    }

    pub(crate) fn maybe_load_schema_for_connection(&mut self, idx: usize, cx: &mut Context<Self>) {
        let Some(ent) = self.registry.read(cx).connections().get(idx).cloned() else {
            return;
        };
        let entry = ent.read(cx);
        let conn_id = entry.id.clone();
        if self
            .conn_states
            .get(&conn_id)
            .is_some_and(|s| s.loading || s.objects.is_some())
        {
            return;
        }
        if let ConnectionState::Connected(ac) = &entry.state {
            self.load_objects_for_connection(idx, ac.clone(), cx);
        }
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
        self.bump_object_list_epoch(cx);
        let conn_ent = self.registry.read(cx).connections()[idx].clone();
        if matches!(conn_ent.read(cx).state, ConnectionState::Connected(_)) {
            self.pending_open_connection = Some(idx);
        }
        cx.notify();
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

        let browser_list = browser_list::ensure_browser_list(self, window, cx);
        browser_list::refresh_browser_list(self, cx);

        v_flex().size_full().min_h_0().child(
            gpui_component::list::List::new(&browser_list)
                .flex_1()
                .min_h_0()
                .w_full()
                .search_placeholder("Search connections & objects"),
        )
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
