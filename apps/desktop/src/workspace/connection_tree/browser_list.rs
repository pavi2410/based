//! Flat browser tree: connections with nested object rows when expanded.

use std::collections::hash_map::DefaultHasher;
use std::hash::{Hash, Hasher};

use gpui::{
    App, Context, ElementId, Entity, IntoElement, MouseButton, ParentElement, RenderOnce,
    SharedString, Task, WeakEntity, Window, div, prelude::*, px,
};
use gpui_component::{
    ActiveTheme, IconName, IndexPath, Selectable, Sizable as _,
    button::{Button, ButtonVariants},
    h_flex,
    list::{ListDelegate, ListState},
    menu::ContextMenuExt,
};

use crate::app::prefs;
use crate::connection::EngineKind;
use crate::widgets::empty_state::empty_state;
use crate::widgets::list_row::{SchemaRowStyle, schema_object_row};
use crate::widgets::{
    BrowserTreeIndent, CONNECTION_CHEVRON_SLOT_W, SIDEBAR_INSET, sidebar_row_inner_gap,
    sidebar_row_padding_y,
};

use super::ConnectionTree;
use super::connection_list::build_connection_rows;
use super::context_menu::{connection_is_connected, object_context_menu, schema_context_menu};
use super::object_list::{group_postgres_objects, object_matches_query};
use super::types::{ConnCache, ConnState, SchemaObject};
use crate::connection::ConnectionState::Connected;

const DEPTH_SCHEMA: u32 = 0;
const DEPTH_KIND: u32 = 1;

#[derive(Clone)]
pub(crate) enum BrowserRow {
    Status {
        conn_idx: usize,
        message: SharedString,
        depth: u32,
    },
    Schema {
        conn_idx: usize,
        name: SharedString,
        expanded: bool,
    },
    Object {
        conn_idx: usize,
        object: SchemaObject,
        depth: u32,
        bare_label: bool,
    },
}

pub(crate) struct BrowserListDelegate {
    tree: Entity<ConnectionTree>,
    rows: Vec<BrowserRow>,
    query: String,
    selected_index: Option<IndexPath>,
}

impl BrowserListDelegate {
    pub(crate) fn new(tree: Entity<ConnectionTree>) -> Self {
        Self {
            tree,
            rows: Vec::new(),
            query: String::new(),
            selected_index: None,
        }
    }

    pub(crate) fn rebuild(&mut self, tree: &ConnectionTree, cx: &App) {
        let q = self.query.trim().to_lowercase();
        let Some(sel) = tree.selected_connection else {
            self.rows.clear();
            self.selected_index = None;
            return;
        };
        let Some(conn) = build_connection_rows(tree, cx)
            .into_iter()
            .find(|c| c.idx == sel)
        else {
            self.rows.clear();
            self.selected_index = None;
            return;
        };

        let conn_id = tree
            .registry
            .read(cx)
            .connections()
            .get(conn.idx)
            .map(|e| e.read(cx).id.clone());
        let Some(conn_id) = conn_id else {
            self.rows.clear();
            self.selected_index = None;
            return;
        };

        let mut rows = Vec::new();
        if conn.is_connecting {
            rows.push(BrowserRow::Status {
                conn_idx: conn.idx,
                message: "Connecting…".into(),
                depth: DEPTH_SCHEMA,
            });
        } else if conn.is_failed {
            rows.push(BrowserRow::Status {
                conn_idx: conn.idx,
                message: conn
                    .fail_reason
                    .as_deref()
                    .map(super::notify::error_one_liner)
                    .unwrap_or_else(|| "Could not connect".into()),
                depth: DEPTH_SCHEMA,
            });
        } else if !conn.is_connected {
            rows.push(BrowserRow::Status {
                conn_idx: conn.idx,
                message: "Not connected".into(),
                depth: DEPTH_SCHEMA,
            });
        } else if let Some(st) = tree.conn_states.get(&conn_id) {
            match st.cache() {
                ConnCache::Loading | ConnCache::Idle => {
                    rows.push(BrowserRow::Status {
                        conn_idx: conn.idx,
                        message: "Loading objects…".into(),
                        depth: DEPTH_SCHEMA,
                    });
                }
                ConnCache::Error(err) => {
                    rows.push(BrowserRow::Status {
                        conn_idx: conn.idx,
                        message: super::notify::error_one_liner(err),
                        depth: DEPTH_SCHEMA,
                    });
                }
                ConnCache::Ready(objects) => match conn.engine {
                    EngineKind::Postgres => {
                        push_postgres_rows(conn.idx, objects, st, &q, &mut rows);
                    }
                    _ => {
                        push_kind_rows(conn.idx, objects, &q, &mut rows, false);
                    }
                },
            }
        }

        self.rows = rows;
        self.selected_index = tree.selected_object.as_ref().and_then(|name| {
            self.rows
                .iter()
                .position(|r| {
                    matches!(
                        r,
                        BrowserRow::Object { object, .. } if object.display_name() == *name
                    )
                })
                .map(IndexPath::new)
        });
    }

    fn row_at(&self, ix: IndexPath) -> Option<&BrowserRow> {
        self.rows.get(ix.row)
    }

    pub(crate) fn object_count(&self) -> usize {
        self.rows
            .iter()
            .filter(|r| matches!(r, BrowserRow::Object { .. }))
            .count()
    }
}

fn push_postgres_rows(
    conn_idx: usize,
    objects: &[SchemaObject],
    state: &ConnState,
    q: &str,
    child_rows: &mut Vec<BrowserRow>,
) -> bool {
    let ConnState::Postgres {
        expanded_schemas, ..
    } = state
    else {
        return push_kind_rows(conn_idx, objects, q, child_rows, false);
    };

    let mut child_matches = false;
    for schema_section in group_postgres_objects(objects.to_vec()) {
        let schema_name = schema_section.name.to_string();
        let schema_hit = q.is_empty() || schema_name.to_lowercase().contains(q);
        let object_hit = schema_section.kinds.iter().any(|kind| {
            kind.items
                .iter()
                .any(|object| object_matches_query(object, q))
        });
        if !q.is_empty() && !schema_hit && !object_hit {
            continue;
        }

        child_matches = true;
        let schema_expanded = (!q.is_empty() && (schema_hit || object_hit))
            || expanded_schemas.contains(&schema_name);

        child_rows.push(BrowserRow::Schema {
            conn_idx,
            name: schema_section.name.clone(),
            expanded: schema_expanded,
        });

        if schema_expanded {
            let mut objects: Vec<SchemaObject> = schema_section
                .kinds
                .into_iter()
                .flat_map(|kind| kind.items)
                .collect();
            objects.sort_by(|a, b| a.name.cmp(&b.name));
            for object in objects {
                if q.is_empty() || schema_hit || object_matches_query(&object, q) {
                    child_rows.push(BrowserRow::Object {
                        conn_idx,
                        object,
                        depth: DEPTH_KIND,
                        bare_label: true,
                    });
                }
            }
        }
    }
    child_matches
}

fn push_kind_rows(
    conn_idx: usize,
    objects: &[SchemaObject],
    q: &str,
    child_rows: &mut Vec<BrowserRow>,
    bare_label: bool,
) -> bool {
    let mut items: Vec<SchemaObject> = objects
        .iter()
        .filter(|object| object_matches_query(object, q))
        .cloned()
        .collect();
    items.sort_by(|a, b| a.name.cmp(&b.name));
    let child_matches = !items.is_empty();
    for object in items {
        child_rows.push(BrowserRow::Object {
            conn_idx,
            object,
            depth: DEPTH_SCHEMA,
            bare_label,
        });
    }
    child_matches
}

#[derive(IntoElement)]
pub(crate) struct BrowserRowItem {
    id: ElementId,
    row: BrowserRow,
    selected: bool,
    tree: WeakEntity<ConnectionTree>,
}

impl Selectable for BrowserRowItem {
    fn selected(mut self, selected: bool) -> Self {
        self.selected = selected;
        self
    }

    fn is_selected(&self) -> bool {
        self.selected
    }
}

impl RenderOnce for BrowserRowItem {
    fn render(self, _window: &mut Window, cx: &mut App) -> impl IntoElement {
        let indent = BrowserTreeIndent::from_app(cx);
        match self.row {
            BrowserRow::Status { message, depth, .. } => {
                status_row(message, depth, &indent, cx).into_any_element()
            }
            BrowserRow::Schema {
                conn_idx,
                name,
                expanded,
            } => schema_row_element(conn_idx, name, expanded, self.tree, &indent, cx)
                .into_any_element(),
            BrowserRow::Object {
                conn_idx,
                object,
                depth,
                bare_label,
                ..
            } => {
                let (engine, is_connected) = self
                    .tree
                    .upgrade()
                    .and_then(|tree| {
                        tree.read(cx)
                            .registry
                            .read(cx)
                            .connections()
                            .get(conn_idx)
                            .map(|e| {
                                let entry = e.read(cx);
                                (entry.config.engine(), matches!(entry.state, Connected(_)))
                            })
                    })
                    .unwrap_or((EngineKind::Postgres, false));
                object_row_element(
                    conn_idx,
                    object,
                    bare_label,
                    depth,
                    self.selected,
                    engine,
                    is_connected,
                    self.tree.clone(),
                    &indent,
                    cx,
                )
                .into_any_element()
            }
        }
    }
}

fn status_row(
    message: SharedString,
    depth: u32,
    indent: &BrowserTreeIndent,
    cx: &App,
) -> impl IntoElement {
    div()
        .pl(px(indent.pl(depth)))
        .pr(px(SIDEBAR_INSET))
        .py(px(sidebar_row_padding_y(cx)))
        .text_xs()
        .text_color(cx.theme().muted_foreground)
        .child(message)
}

fn schema_row_element(
    conn_idx: usize,
    name: SharedString,
    expanded: bool,
    tree: WeakEntity<ConnectionTree>,
    indent: &BrowserTreeIndent,
    cx: &mut App,
) -> impl IntoElement {
    let tree_chevron = tree.clone();
    let schema_name = name.to_string();
    let schema_menu_name = schema_name.clone();
    let schema_key = {
        let mut hasher = DefaultHasher::new();
        conn_idx.hash(&mut hasher);
        name.hash(&mut hasher);
        hasher.finish()
    };

    let chevron = Button::new(("browser-schema-chevron", schema_key))
        .ghost()
        .xsmall()
        .icon(if expanded {
            IconName::ChevronDown
        } else {
            IconName::ChevronRight
        })
        .on_mouse_down(MouseButton::Left, move |_, _, cx| {
            cx.stop_propagation();
            if let Some(ent) = tree_chevron.upgrade() {
                ent.update(cx, |t, cx| {
                    let Some(conn_id) = t
                        .registry
                        .read(cx)
                        .connections()
                        .get(conn_idx)
                        .map(|e| e.read(cx).id.clone())
                    else {
                        return;
                    };
                    t.toggle_schema_expanded(conn_id, schema_name.clone(), cx);
                });
            }
        });

    let tree_menu = tree.clone();
    let is_connected = tree
        .upgrade()
        .map(|t| connection_is_connected(t.read(cx), conn_idx, cx))
        .unwrap_or(false);

    h_flex()
        .w_full()
        .context_menu(move |menu, _window, cx| {
            schema_context_menu(
                conn_idx,
                schema_menu_name.clone(),
                expanded,
                is_connected,
                tree_menu.clone(),
                menu,
                cx,
            )
        })
        .pl(px(indent.pl(DEPTH_SCHEMA)))
        .pr(px(SIDEBAR_INSET))
        .py(px(sidebar_row_padding_y(cx)))
        .gap(px(sidebar_row_inner_gap(cx)))
        .items_center()
        .child(
            h_flex()
                .w(px(CONNECTION_CHEVRON_SLOT_W))
                .flex_shrink_0()
                .items_center()
                .justify_center()
                .child(chevron),
        )
        .child(div().flex_1().min_w_0().text_sm().truncate().child(name))
}

fn object_row_key(object: &SchemaObject) -> u64 {
    let mut hasher = DefaultHasher::new();
    object.display_name().hash(&mut hasher);
    object.kind.label().hash(&mut hasher);
    hasher.finish()
}

#[allow(clippy::too_many_arguments)]
fn object_row_element(
    conn_idx: usize,
    object: SchemaObject,
    bare_label: bool,
    depth: u32,
    selected: bool,
    engine: EngineKind,
    is_connected: bool,
    tree: WeakEntity<ConnectionTree>,
    indent: &BrowserTreeIndent,
    cx: &App,
) -> impl IntoElement {
    let muted = cx.theme().muted_foreground;
    let fg = cx.theme().foreground;
    let style = SchemaRowStyle {
        muted,
        fg,
        icon_color: object.kind.accent_color(cx.theme()),
        mono_family: prefs::code_font_family(cx),
        row_py: sidebar_row_padding_y(cx),
        row_gap: sidebar_row_inner_gap(cx),
    };
    let label: SharedString = if bare_label {
        object.name.clone().into()
    } else {
        object.display_name().into()
    };
    let tree_menu = tree.clone();
    let object_menu = object.clone();
    div()
        .w_full()
        .context_menu(move |menu, _window, cx| {
            object_context_menu(
                conn_idx,
                object_menu.clone(),
                engine,
                is_connected,
                tree_menu.clone(),
                menu,
                cx,
            )
        })
        .child(
            schema_object_row(
                ("browser-obj", object_row_key(&object)),
                selected,
                object.kind.list_icon(),
                label,
                style,
            )
            .pl(px(indent.pl(depth)))
            .pr(px(SIDEBAR_INSET)),
        )
}

impl ListDelegate for BrowserListDelegate {
    type Item = BrowserRowItem;

    fn items_count(&self, _section: usize, _: &App) -> usize {
        self.rows.len()
    }

    fn perform_search(
        &mut self,
        query: &str,
        _window: &mut Window,
        cx: &mut Context<ListState<Self>>,
    ) -> Task<()> {
        self.query = query.to_string();
        let tree = self.tree.read(cx);
        self.rebuild(tree, cx);
        Task::ready(())
    }

    fn render_item(
        &mut self,
        ix: IndexPath,
        _window: &mut Window,
        _cx: &mut Context<ListState<Self>>,
    ) -> Option<Self::Item> {
        let row = self.row_at(ix)?.clone();
        let selected = self.selected_index == Some(ix);
        let id: ElementId = ("browser-row", ix.row).into();
        Some(BrowserRowItem {
            id,
            row,
            selected,
            tree: self.tree.downgrade(),
        })
    }

    fn render_empty(
        &mut self,
        _window: &mut Window,
        cx: &mut Context<ListState<Self>>,
    ) -> impl IntoElement {
        let searching = !self.query.trim().is_empty();
        let has_selection = self.tree.read(cx).selected_connection.is_some();
        let (title, body) = if !has_selection {
            ("No connections", "Add a connection to this project.")
        } else if searching {
            ("No matches", "No objects match your search.")
        } else {
            ("Empty catalog", "No objects in this catalog.")
        };
        empty_state(title, body, IconName::Search, cx)
    }

    fn set_selected_index(
        &mut self,
        ix: Option<IndexPath>,
        _window: &mut Window,
        cx: &mut Context<ListState<Self>>,
    ) {
        self.selected_index = ix;
        let selected = ix.and_then(|i| match self.row_at(i) {
            Some(BrowserRow::Object { conn_idx, .. }) => Some(*conn_idx),
            _ => None,
        });
        self.tree.update(cx, |tree, cx| {
            if let Some(idx) = selected {
                tree.selected_connection = Some(idx);
            }
            cx.notify();
        });
        cx.notify();
    }

    fn confirm(
        &mut self,
        _secondary: bool,
        window: &mut Window,
        cx: &mut Context<ListState<Self>>,
    ) {
        let Some(ix) = self.selected_index else {
            return;
        };
        if let Some(BrowserRow::Object {
            object, conn_idx, ..
        }) = self.row_at(ix).cloned()
        {
            self.tree.update(cx, |tree, cx| {
                tree.selected_connection = Some(conn_idx);
                tree.on_object_clicked(object, window, cx);
            });
        }
    }
}

pub(crate) fn ensure_browser_list(
    tree: &mut ConnectionTree,
    window: &mut Window,
    cx: &mut Context<ConnectionTree>,
) -> Entity<ListState<BrowserListDelegate>> {
    if let Some(list) = tree.browser_list.clone() {
        return list;
    }

    let tree_entity = cx.entity();
    let mut delegate = BrowserListDelegate::new(tree_entity.clone());
    delegate.rebuild(tree, cx);
    let list = cx.new(|cx| {
        ListState::new(delegate, window, cx)
            .searchable(false)
            .selectable(true)
    });

    tree.browser_list = Some(list.clone());
    list
}

pub(crate) fn refresh_browser_list(tree: &ConnectionTree, cx: &mut Context<ConnectionTree>) {
    let Some(list) = tree.browser_list.clone() else {
        return;
    };
    list.update(cx, |list, cx| {
        list.delegate_mut().rebuild(tree, cx);
        cx.notify();
    });
}

pub(crate) fn apply_catalog_query(
    tree: &ConnectionTree,
    query: &str,
    cx: &mut Context<ConnectionTree>,
) {
    let Some(list) = tree.browser_list.clone() else {
        return;
    };
    list.update(cx, |list, cx| {
        list.delegate_mut().query = query.to_string();
        list.delegate_mut().rebuild(tree, cx);
        cx.notify();
    });
}
