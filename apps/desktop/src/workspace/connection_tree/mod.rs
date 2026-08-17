// Sidebar connection list + object browser (extracted from workspace).

use std::collections::HashMap;
use std::mem;

use gpui::{
    App, ClipboardItem, Context, Entity, EventEmitter, Focusable, IntoElement, Render, Window,
    prelude::*,
};
use gpui_component::{
    dock::DockArea,
    h_flex,
    input::{InputEvent, InputState},
    list::ListState,
};

use crate::connection::registry::{ConnectionRegistry, RegistryEvent};
use crate::connection::{
    AnyConnection, ConnectionEntry, ConnectionId, ConnectionState, EngineKind,
};

use super::notify;
use crate::connection::close_any_connection;
use crate::workspace::TabSpec;

mod browser_list;
mod connect;
mod connection_list;
mod content_rail;
mod context_menu;
mod icon_rail;
mod object_list;
mod open_workspace;
mod schema_load;
mod types;

pub(crate) use context_menu::connection_actions_menu;
pub(crate) use icon_rail::ICON_RAIL_WIDTH;
pub use types::{ObjectKind, SchemaObject, TreeEvent};

use types::{ActiveObjects, ConnState};

pub struct ConnectionTree {
    pub registry: Entity<ConnectionRegistry>,
    dock_area: Entity<DockArea>,
    conn_states: HashMap<ConnectionId, ConnState>,
    #[allow(dead_code)]
    active_spec: Option<TabSpec>,
    pub(crate) selected_connection: Option<usize>,
    pub(crate) active_objects: ActiveObjects,
    pub(crate) selected_object: Option<String>,
    pub(crate) browser_list: Option<Entity<ListState<browser_list::BrowserListDelegate>>>,
    object_list_epoch: u64,
    pending_open_connection: Option<usize>,
    pub(crate) catalog_search_open: bool,
    pub(crate) queries_search_open: bool,
    pub(crate) catalog_collapsed: bool,
    pub(crate) queries_collapsed: bool,
    pub(crate) catalog_search: Option<Entity<InputState>>,
    pub(crate) queries_search: Option<Entity<InputState>>,
    pub(crate) content_rail_expanded: bool,
}

impl ConnectionTree {
    pub(crate) fn queue_open_connected(&mut self, conn_id: &ConnectionId, cx: &mut Context<Self>) {
        let idx = self
            .registry
            .read(cx)
            .connections()
            .iter()
            .position(|e| e.read(cx).id == *conn_id);
        let Some(idx) = idx else {
            return;
        };
        self.selected_connection = Some(idx);
        self.content_rail_expanded = true;
        self.pending_open_connection = Some(idx);
        cx.notify();
    }

    pub fn new(
        registry: Entity<ConnectionRegistry>,
        dock_area: Entity<DockArea>,
        cx: &mut Context<Self>,
    ) -> Self {
        cx.subscribe(&registry, |this, _, event, cx| match event {
            RegistryEvent::Added(id) => {
                let engine = this
                    .registry
                    .read(cx)
                    .get(id, cx)
                    .map(|e| e.read(cx).config.engine())
                    .unwrap_or(EngineKind::Postgres);
                this.conn_states
                    .entry(id.clone())
                    .or_insert_with(|| ConnState::new(engine));
                cx.notify();
            }
            RegistryEvent::Removed(id) => {
                this.conn_states.remove(id);
                this.selected_connection = None;
                this.active_objects = ActiveObjects::Empty;
                this.selected_object = None;
                this.bump_object_list_epoch(cx);
                this.pending_open_connection = None;
                this.content_rail_expanded = false;
                cx.notify();
            }
            RegistryEvent::StateChanged(_) => cx.notify(),
        })
        .detach();

        let mut conn_states = HashMap::new();
        for ent in registry.read(cx).connections().iter() {
            let entry = ent.read(cx);
            conn_states.insert(entry.id.clone(), ConnState::new(entry.config.engine()));
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
            object_list_epoch: 0,
            pending_open_connection: None,
            catalog_search_open: false,
            queries_search_open: false,
            catalog_collapsed: false,
            queries_collapsed: false,
            catalog_search: None,
            queries_search: None,
            content_rail_expanded: false,
        }
    }

    pub(crate) fn bump_object_list_epoch(&mut self, cx: &mut Context<Self>) {
        self.object_list_epoch = self.object_list_epoch.wrapping_add(1);
        browser_list::refresh_browser_list(self, cx);
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
            st.set_expanded(expanded);
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
            .is_some_and(|s| s.cache().should_skip_load())
        {
            return;
        }
        if let ConnectionState::Connected(ac) = &entry.state {
            self.load_objects_for_connection(idx, ac.clone(), cx);
        }
    }

    pub(crate) fn toggle_schema_expanded(
        &mut self,
        conn_id: ConnectionId,
        schema: String,
        cx: &mut Context<Self>,
    ) {
        let Some(st) = self.conn_states.get_mut(&conn_id) else {
            return;
        };
        let Some(schemas) = st.postgres_schemas() else {
            return;
        };
        if schemas.contains(&schema) {
            schemas.remove(&schema);
        } else {
            schemas.insert(schema);
        }
        self.bump_object_list_epoch(cx);
    }

    pub(crate) fn open_new_query(&mut self, idx: usize, cx: &mut Context<Self>) {
        let Some(ent) = self.registry.read(cx).connections().get(idx) else {
            return;
        };
        let conn_id = ent.read(cx).id.clone();
        cx.emit(TreeEvent::OpenTab(TabSpec::blank_query_editor(conn_id)));
    }

    /// Clears cached schema objects and refetches when the connection is open.
    pub(crate) fn refresh_connection(&mut self, idx: usize, cx: &mut Context<Self>) {
        let Some(ent) = self.registry.read(cx).connections().get(idx).cloned() else {
            return;
        };
        let entry = ent.read(cx);
        let conn_id = entry.id.clone();
        if let Some(st) = self.conn_states.get_mut(&conn_id) {
            st.cache_mut().invalidate();
        }
        if let ConnectionState::Connected(ac) = &entry.state {
            self.load_objects_for_connection(idx, ac.clone(), cx);
        } else {
            self.bump_object_list_epoch(cx);
        }
    }

    pub(crate) fn disconnect_at(&mut self, idx: usize, cx: &mut Context<Self>) {
        let ent = self.registry.read(cx).connections().get(idx).cloned();
        let Some(ent) = ent else {
            return;
        };
        ent.update(cx, |e, cx| {
            if let ConnectionState::Connected(ac) =
                mem::replace(&mut e.state, ConnectionState::Disconnected)
            {
                close_any_connection(ac, cx);
            }
            e.last_error = None;
            cx.notify();
        });
        cx.notify();
    }

    pub(crate) fn ensure_selected_connection(&mut self, cx: &App) {
        let n = self.registry.read(cx).connections().len();
        if n == 0 {
            self.selected_connection = None;
            return;
        }
        if let Some(idx) = self.selected_connection
            && idx >= n
        {
            self.selected_connection = None;
        }
    }

    pub(crate) fn selected_connection_id(&self, cx: &App) -> Option<ConnectionId> {
        self.selected_connection_entry(cx)
            .map(|entry| entry.read(cx).id.clone())
    }

    fn ensure_search_inputs(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if self.catalog_search.is_none() {
            let input = cx.new(|cx| InputState::new(window, cx).placeholder("Search catalog"));
            cx.subscribe_in(&input, window, |this, input, event, _window, cx| {
                if matches!(event, InputEvent::Change) {
                    let q = input.read(cx).value().to_string();
                    browser_list::apply_catalog_query(this, &q, cx);
                    cx.notify();
                }
            })
            .detach();
            self.catalog_search = Some(input);
        }
        if self.queries_search.is_none() {
            let input = cx.new(|cx| InputState::new(window, cx).placeholder("Search queries"));
            cx.subscribe_in(&input, window, |_this, _input, event, _window, cx| {
                if matches!(event, InputEvent::Change) {
                    cx.notify();
                }
            })
            .detach();
            self.queries_search = Some(input);
        }
    }

    pub(crate) fn toggle_pane_collapsed(&mut self, is_catalog: bool, cx: &mut Context<Self>) {
        if is_catalog {
            self.catalog_collapsed = !self.catalog_collapsed;
            if self.catalog_collapsed {
                self.catalog_search_open = false;
            }
        } else {
            self.queries_collapsed = !self.queries_collapsed;
            if self.queries_collapsed {
                self.queries_search_open = false;
            }
        }
        cx.notify();
    }

    pub(crate) fn toggle_pane_search(
        &mut self,
        is_catalog: bool,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if is_catalog {
            self.catalog_collapsed = false;
            self.catalog_search_open = !self.catalog_search_open;
            if !self.catalog_search_open {
                if let Some(input) = self.catalog_search.clone() {
                    input.update(cx, |input, cx| input.set_value("", window, cx));
                }
                browser_list::apply_catalog_query(self, "", cx);
            } else if let Some(input) = self.catalog_search.clone() {
                input.read(cx).focus_handle(cx).focus(window, cx);
            }
        } else {
            self.queries_collapsed = false;
            self.queries_search_open = !self.queries_search_open;
            if !self.queries_search_open {
                if let Some(input) = self.queries_search.clone() {
                    input.update(cx, |input, cx| input.set_value("", window, cx));
                }
            } else if let Some(input) = self.queries_search.clone() {
                input.read(cx).focus_handle(cx).focus(window, cx);
            }
        }
        cx.notify();
    }

    pub(crate) fn toggle_content_rail(&mut self, cx: &mut Context<Self>) {
        self.content_rail_expanded = !self.content_rail_expanded;
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
        self.content_rail_expanded = true;
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
        self.open_data_tab(object, idx, cx);
        cx.notify();
    }

    pub(crate) fn open_data_tab(
        &mut self,
        object: SchemaObject,
        conn_idx: usize,
        cx: &mut Context<Self>,
    ) {
        self.selected_connection = Some(conn_idx);
        self.selected_object = Some(object.display_name());
        let Some(ent) = self.registry.read(cx).connections().get(conn_idx).cloned() else {
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
                _ => self.emit_object_info_tab(object, conn_idx, cx),
            },
            Some(AnyConnection::Postgres(_)) => match object.kind {
                ObjectKind::Table | ObjectKind::View | ObjectKind::MaterializedView => {
                    cx.emit(TreeEvent::OpenTab(TabSpec::DataViewer {
                        conn_id: ent.read(cx).id.clone(),
                        object: object.display_name(),
                    }));
                }
                _ => self.emit_object_info_tab(object, conn_idx, cx),
            },
            Some(AnyConnection::MongoDB(_)) => {
                if matches!(object.kind, ObjectKind::Collection) {
                    cx.emit(TreeEvent::OpenTab(TabSpec::DataViewer {
                        conn_id: ent.read(cx).id.clone(),
                        object: object.display_name(),
                    }));
                } else {
                    self.emit_object_info_tab(object, conn_idx, cx);
                }
            }
            _ => {}
        }
    }

    pub(crate) fn open_structure_tab(
        &mut self,
        object: SchemaObject,
        conn_idx: usize,
        cx: &mut Context<Self>,
    ) {
        self.selected_connection = Some(conn_idx);
        self.selected_object = Some(object.display_name());
        let Some(ent) = self.registry.read(cx).connections().get(conn_idx).cloned() else {
            return;
        };
        if !matches!(ent.read(cx).state, ConnectionState::Connected(_)) {
            return;
        }
        let engine = ent.read(cx).config.engine();
        let supported = match engine {
            EngineKind::Postgres => matches!(
                object.kind,
                ObjectKind::Table | ObjectKind::View | ObjectKind::MaterializedView
            ),
            EngineKind::SQLite => matches!(object.kind, ObjectKind::Table | ObjectKind::View),
            EngineKind::MongoDB => false,
        };
        if !supported {
            notify::push_info(cx, "Structure view is not available for this object yet.");
            return;
        }
        let conn_id = ent.read(cx).id.clone();
        self.open_inspector_tab(object, conn_id, cx);
    }

    pub(crate) fn copy_object_name(&self, object: &SchemaObject, cx: &mut App) {
        cx.write_to_clipboard(ClipboardItem::new_string(object.display_name()));
    }

    fn emit_object_info_tab(
        &mut self,
        object: SchemaObject,
        conn_idx: usize,
        cx: &mut Context<Self>,
    ) {
        let Some(ent) = self.registry.read(cx).connections().get(conn_idx).cloned() else {
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

        self.ensure_selected_connection(cx);
        if let Some(idx) = self.selected_connection {
            self.maybe_load_schema_for_connection(idx, cx);
        }
        self.ensure_search_inputs(window, cx);

        let tree_entity = cx.entity().downgrade();
        let rail = icon_rail::render_icon_rail(
            tree_entity.clone(),
            self.selected_connection,
            self.content_rail_expanded,
            connection_list::build_connection_rows(self, cx),
            cx,
        );
        let browser_list = browser_list::ensure_browser_list(self, window, cx);
        browser_list::refresh_browser_list(self, cx);

        h_flex()
            .size_full()
            .min_h_0()
            .items_start()
            .child(rail)
            .when(self.content_rail_expanded, |row| {
                row.child(content_rail::render_content_rail(
                    self,
                    tree_entity,
                    browser_list,
                    cx,
                ))
            })
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
            let Some(objects) = state.cache().objects() else {
                continue;
            };
            for obj in objects {
                let name = obj.display_name();
                let bare = obj.name.to_lowercase();
                let schema = obj.schema.as_ref().map(|s| s.to_lowercase());
                if q.is_empty()
                    || name.to_lowercase().contains(&q)
                    || bare.contains(&q)
                    || schema.is_some_and(|s| s.contains(&q))
                    || conn_id.0.to_lowercase().contains(&q)
                {
                    out.push((conn_id.clone(), obj.clone(), engine));
                }
            }
        }
        out
    }
}
