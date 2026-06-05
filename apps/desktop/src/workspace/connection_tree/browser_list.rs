//! Flat browser tree: connections with nested object rows when expanded.

use std::collections::hash_map::DefaultHasher;
use std::hash::{Hash, Hasher};

use gpui::{
    App, Context, ElementId, Entity, IntoElement, MouseButton, ParentElement, RenderOnce,
    SharedString, Task, WeakEntity, Window, div, prelude::*, px,
};
use gpui_component::{
    ActiveTheme, IconName, IndexPath, Selectable, Sizable as _, StyledExt,
    button::{Button, ButtonVariants},
    h_flex,
    list::ListItem,
    list::{ListDelegate, ListState},
    menu::{ContextMenuExt, PopupMenuItem},
};

use crate::app::prefs;
use crate::connection::EngineKind;
use crate::widgets::empty_state::empty_state;
use crate::widgets::list_row::{SchemaRowStyle, schema_object_row};
use crate::widgets::{
    BrowserTreeIndent, CONNECTION_CHEVRON_SLOT_W, SIDEBAR_INSET, engine_icon,
    sidebar_row_inner_gap, sidebar_row_padding_y,
};

use super::ConnectionTree;
use super::connection_list::{
    ConnectionRow, build_connection_rows, connection_row_status_indicator,
};
use super::object_list::{group_by_kind, group_postgres_objects, object_matches_query};
use super::types::{ConnCache, ConnState, SchemaObject};

const DEPTH_SCHEMA: u32 = 1;
const DEPTH_KIND: u32 = 2;
const DEPTH_LEAF: u32 = 3;

#[derive(Clone)]
pub(crate) enum BrowserRow {
    Connection(ConnectionRow),
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
    Section {
        conn_idx: usize,
        title: SharedString,
        depth: u32,
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
        let all_connections = build_connection_rows(tree, cx);
        let mut rows = Vec::new();

        for conn in all_connections {
            let conn_id = tree
                .registry
                .read(cx)
                .connections()
                .get(conn.idx)
                .map(|e| e.read(cx).id.clone());
            let Some(conn_id) = conn_id else { continue };

            let conn_matches = q.is_empty()
                || conn.conn_label.to_lowercase().contains(&q)
                || conn.engine.short_label().contains(&q);

            let state = tree.conn_states.get(&conn_id);
            let expanded = state.is_some_and(|s| s.expanded());

            let mut child_matches = false;
            let mut child_rows = Vec::new();
            if expanded && let Some(st) = state {
                match st.cache() {
                    ConnCache::Loading => {
                        child_rows.push(BrowserRow::Status {
                            conn_idx: conn.idx,
                            message: "Loading objects…".into(),
                            depth: DEPTH_SCHEMA,
                        });
                        child_matches = true;
                    }
                    ConnCache::Error(err) => {
                        child_rows.push(BrowserRow::Status {
                            conn_idx: conn.idx,
                            message: super::notify::error_one_liner(err),
                            depth: DEPTH_SCHEMA,
                        });
                        child_matches = true;
                    }
                    ConnCache::Ready(objects) => {
                        child_matches = match conn.engine {
                            EngineKind::Postgres => {
                                push_postgres_rows(conn.idx, objects, st, &q, &mut child_rows)
                            }
                            _ => push_kind_rows(conn.idx, objects, &q, &mut child_rows, false),
                        };
                    }
                    ConnCache::Idle => {}
                }
            }

            if conn_matches || child_matches {
                rows.push(BrowserRow::Connection(conn));
                rows.extend(child_rows);
            }
        }

        self.rows = rows;
        if let Some(sel) = tree.selected_connection {
            self.selected_index = self
                .rows
                .iter()
                .position(|r| matches!(r, BrowserRow::Connection(c) if c.idx == sel))
                .map(IndexPath::new);
        }
    }

    fn row_at(&self, ix: IndexPath) -> Option<&BrowserRow> {
        self.rows.get(ix.row)
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
            for kind in schema_section.kinds {
                let kind_hit = q.is_empty() || kind.name.to_lowercase().contains(q);
                let mut kind_rows = Vec::new();
                for object in kind.items {
                    if object_matches_query(&object, q) {
                        kind_rows.push(BrowserRow::Object {
                            conn_idx,
                            object,
                            depth: DEPTH_LEAF,
                            bare_label: true,
                        });
                    }
                }
                if kind_hit || !kind_rows.is_empty() {
                    child_rows.push(BrowserRow::Section {
                        conn_idx,
                        title: kind.name,
                        depth: DEPTH_KIND,
                    });
                    child_rows.extend(kind_rows);
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
    let mut child_matches = false;
    for section in group_by_kind(objects.to_vec()) {
        let section_hit = q.is_empty() || section.name.to_lowercase().contains(q);
        let mut section_rows = Vec::new();
        for object in section.items {
            if object_matches_query(&object, q) {
                section_rows.push(BrowserRow::Object {
                    conn_idx,
                    object,
                    depth: DEPTH_KIND,
                    bare_label,
                });
            }
        }
        if section_hit || !section_rows.is_empty() {
            child_matches = true;
            child_rows.push(BrowserRow::Section {
                conn_idx,
                title: section.name,
                depth: DEPTH_SCHEMA,
            });
            child_rows.extend(section_rows);
        }
    }
    child_matches
}

#[derive(IntoElement)]
pub(crate) struct BrowserRowItem {
    id: ElementId,
    row: BrowserRow,
    selected: bool,
    conn_expanded: bool,
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
            BrowserRow::Connection(row) => {
                connection_row_element(row, self.selected, self.conn_expanded, self.tree, cx)
                    .into_any_element()
            }
            BrowserRow::Status { message, depth, .. } => {
                status_row(message, depth, &indent, cx).into_any_element()
            }
            BrowserRow::Schema {
                conn_idx,
                name,
                expanded,
            } => schema_row_element(conn_idx, name, expanded, self.tree, &indent, cx)
                .into_any_element(),
            BrowserRow::Section { title, depth, .. } => {
                section_row(title, depth, &indent, cx).into_any_element()
            }
            BrowserRow::Object {
                object,
                depth,
                bare_label,
                ..
            } => object_row_element(object, bare_label, depth, self.selected, &indent, cx)
                .into_any_element(),
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

fn section_row(
    title: SharedString,
    depth: u32,
    indent: &BrowserTreeIndent,
    cx: &App,
) -> impl IntoElement {
    let muted = cx.theme().muted_foreground;
    h_flex()
        .pl(px(indent.pl(depth)))
        .pr(px(SIDEBAR_INSET))
        .py(px(sidebar_row_padding_y(cx)))
        .items_center()
        .child(
            div()
                .text_xs()
                .font_bold()
                .font_family(prefs::ui_font_family(cx))
                .font_weight(prefs::ui_font_weight(cx))
                .text_color(muted.opacity(0.86))
                .child(title),
        )
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

    h_flex()
        .w_full()
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

fn connection_row_element(
    row: ConnectionRow,
    selected: bool,
    expanded: bool,
    tree: WeakEntity<ConnectionTree>,
    cx: &mut App,
) -> impl IntoElement {
    let ConnectionRow {
        idx,
        conn_label,
        state_label: _,
        engine,
        state_color,
        is_connected,
        is_connecting,
        is_failed,
        fail_reason: _,
    } = row;

    let tree_click = tree.clone();
    let tree_chevron = tree.clone();
    let tree_menu = tree.clone();
    let err_fg = cx.theme().danger_foreground;

    let chevron = Button::new(("browser-chevron", idx))
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
                ent.update(cx, |t, cx| t.toggle_connection_expanded(idx, cx));
            }
        });

    let chevron_lead = h_flex()
        .w(px(CONNECTION_CHEVRON_SLOT_W))
        .flex_shrink_0()
        .items_center()
        .justify_center()
        .when(is_connected, |slot| slot.child(chevron));

    div()
        .w_full()
        .context_menu(move |menu, _window, cx| {
            if let Some(tree_ent) = tree_menu.upgrade() {
                tree_ent.update(cx, |tree, cx| {
                    tree.selected_connection = Some(idx);
                    cx.notify();
                });
            }
            let mut menu = menu.item(PopupMenuItem::new("New Query").on_click({
                let tree = tree_menu.clone();
                move |_, _window, cx| {
                    if let Some(tree_ent) = tree.upgrade() {
                        tree_ent.update(cx, |tree, cx| {
                            tree.open_new_query(idx, cx);
                        });
                    }
                }
            }));
            if is_connected {
                menu = menu.item(PopupMenuItem::new("Disconnect").on_click({
                    let tree = tree_menu.clone();
                    move |_, _window, cx| {
                        if let Some(tree_ent) = tree.upgrade() {
                            tree_ent.update(cx, |tree, cx| {
                                tree.disconnect_at(idx, cx);
                            });
                        }
                    }
                }));
            }
            menu
        })
        .child(
            ListItem::new(("browser-conn", idx))
                .selected(selected)
                .pl(px(SIDEBAR_INSET))
                .pr(px(SIDEBAR_INSET))
                .py(px(sidebar_row_padding_y(cx)))
                .cursor_pointer()
                .on_mouse_down(MouseButton::Left, move |_, window, cx| {
                    if let Some(ent) = tree_click.upgrade() {
                        ent.update(cx, |t, cx| t.on_connection_row_clicked(idx, window, cx));
                    }
                })
                .child(
                    h_flex()
                        .w_full()
                        .gap(px(sidebar_row_inner_gap(cx)))
                        .items_center()
                        .child(chevron_lead)
                        .child(engine_icon(engine))
                        .child(
                            div()
                                .flex_1()
                                .min_w_0()
                                .text_sm()
                                .truncate()
                                .when(is_failed, |d| d.text_color(err_fg.opacity(0.92)))
                                .child(conn_label),
                        )
                        .child(connection_row_status_indicator(
                            is_connected,
                            is_failed,
                            is_connecting,
                            state_color,
                            err_fg,
                            cx,
                        )),
                ),
        )
}

fn object_row_element(
    object: SchemaObject,
    bare_label: bool,
    depth: u32,
    selected: bool,
    indent: &BrowserTreeIndent,
    cx: &App,
) -> impl IntoElement {
    let muted = cx.theme().muted_foreground;
    let fg = cx.theme().foreground;
    let style = SchemaRowStyle {
        muted,
        fg,
        mono_family: prefs::code_font_family(cx),
        row_py: sidebar_row_padding_y(cx),
        row_gap: sidebar_row_inner_gap(cx),
    };
    let label: SharedString = if bare_label {
        object.name.clone().into()
    } else {
        object.display_name().into()
    };
    schema_object_row(
        ("browser-obj", object_row_key(&object)),
        selected,
        object.kind.list_icon(),
        label,
        style,
    )
    .pl(px(indent.pl(depth)))
    .pr(px(SIDEBAR_INSET))
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
        cx: &mut Context<ListState<Self>>,
    ) -> Option<Self::Item> {
        let row = self.row_at(ix)?.clone();
        let selected = self.selected_index == Some(ix);
        let conn_expanded = match &row {
            BrowserRow::Connection(c) => self
                .tree
                .read(cx)
                .conn_states
                .get(
                    &self
                        .tree
                        .read(cx)
                        .registry
                        .read(cx)
                        .connections()
                        .get(c.idx)?
                        .read(cx)
                        .id,
                )
                .is_some_and(|s| s.expanded()),
            _ => false,
        };
        let id: ElementId = ("browser-row", ix.row).into();
        Some(BrowserRowItem {
            id,
            row,
            selected,
            conn_expanded,
            tree: self.tree.downgrade(),
        })
    }

    fn render_empty(
        &mut self,
        _window: &mut Window,
        cx: &mut Context<ListState<Self>>,
    ) -> impl IntoElement {
        empty_state(
            "No matches",
            "No connections match your search.",
            IconName::Search,
            cx,
        )
    }

    fn set_selected_index(
        &mut self,
        ix: Option<IndexPath>,
        _window: &mut Window,
        cx: &mut Context<ListState<Self>>,
    ) {
        self.selected_index = ix;
        let selected = ix.and_then(|i| match self.row_at(i) {
            Some(BrowserRow::Connection(c)) => Some(c.idx),
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
            .searchable(true)
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
