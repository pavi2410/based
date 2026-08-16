//! Right-click context menus for the connection browser tree.

use gpui::{App, ClipboardItem, WeakEntity};
use gpui_component::menu::{PopupMenu, PopupMenuItem};

use crate::connection::{ConnectionConfig, ConnectionId, ConnectionState, EngineKind};
use crate::mongodb::{mongo_uri, mongosh_command};
use crate::postgres::{postgres_uri, psql_command};
use crate::sqlite::{resolve_sqlite_path, sqlite_uri, sqlite3_command};
use crate::workspace::notify;

use super::ConnectionTree;
use super::types::{ObjectKind, SchemaObject};

fn coming_soon_item(label: &'static str) -> PopupMenuItem {
    PopupMenuItem::new(label).on_click(move |_, _, cx| {
        notify::push_info(cx, format!("{label} — coming soon"));
    })
}

fn copy_config_item(
    label: &'static str,
    tree: WeakEntity<ConnectionTree>,
    conn_id: ConnectionId,
    text: impl Fn(&ConnectionConfig, &App) -> Option<String> + 'static,
) -> PopupMenuItem {
    PopupMenuItem::new(label).on_click(move |_, _, cx| {
        let Some(cfg) = tree.upgrade().and_then(|tree_ent| {
            tree_ent
                .read(cx)
                .registry
                .read(cx)
                .get(&conn_id, cx)
                .map(|entry| entry.read(cx).config.clone())
        }) else {
            return;
        };
        let Some(text) = text(&cfg, cx) else {
            return;
        };
        cx.write_to_clipboard(ClipboardItem::new_string(text));
    })
}

fn add_copy_items(
    menu: PopupMenu,
    tree: WeakEntity<ConnectionTree>,
    conn_id: ConnectionId,
    engine: EngineKind,
) -> PopupMenu {
    match engine {
        EngineKind::Postgres => menu
            .item(copy_config_item(
                "Copy Connection String",
                tree.clone(),
                conn_id.clone(),
                |cfg, _| match cfg {
                    ConnectionConfig::Postgres(c) => Some(postgres_uri(c, false)),
                    _ => None,
                },
            ))
            .item(copy_config_item(
                "Copy Connection String with Password",
                tree.clone(),
                conn_id.clone(),
                |cfg, _| match cfg {
                    ConnectionConfig::Postgres(c) => Some(postgres_uri(c, true)),
                    _ => None,
                },
            ))
            .item(copy_config_item(
                "Copy psql Command",
                tree.clone(),
                conn_id.clone(),
                |cfg, _| match cfg {
                    ConnectionConfig::Postgres(c) => Some(psql_command(c, false)),
                    _ => None,
                },
            ))
            .item(copy_config_item(
                "Copy psql Command with Password",
                tree,
                conn_id.clone(),
                |cfg, _| match cfg {
                    ConnectionConfig::Postgres(c) => Some(psql_command(c, true)),
                    _ => None,
                },
            )),
        EngineKind::SQLite => menu
            .item(copy_config_item(
                "Copy Connection String",
                tree.clone(),
                conn_id.clone(),
                |cfg, cx| match cfg {
                    ConnectionConfig::SQLite(c) => {
                        let path = resolve_sqlite_path(&c.path, cx);
                        Some(sqlite_uri(&path, c.read_only))
                    }
                    _ => None,
                },
            ))
            .item(copy_config_item(
                "Copy sqlite3 Command",
                tree,
                conn_id.clone(),
                |cfg, cx| match cfg {
                    ConnectionConfig::SQLite(c) => {
                        let path = resolve_sqlite_path(&c.path, cx);
                        Some(sqlite3_command(&path, c.read_only))
                    }
                    _ => None,
                },
            )),
        EngineKind::MongoDB => menu
            .item(copy_config_item(
                "Copy Connection String",
                tree.clone(),
                conn_id.clone(),
                |cfg, _| match cfg {
                    ConnectionConfig::MongoDB(c) => Some(mongo_uri(c, false)),
                    _ => None,
                },
            ))
            .item(copy_config_item(
                "Copy Connection String with Password",
                tree.clone(),
                conn_id.clone(),
                |cfg, _| match cfg {
                    ConnectionConfig::MongoDB(c) => Some(mongo_uri(c, true)),
                    _ => None,
                },
            ))
            .item(copy_config_item(
                "Copy mongosh Command",
                tree.clone(),
                conn_id.clone(),
                |cfg, _| match cfg {
                    ConnectionConfig::MongoDB(c) => Some(mongosh_command(c, false)),
                    _ => None,
                },
            ))
            .item(copy_config_item(
                "Copy mongosh Command with Password",
                tree,
                conn_id.clone(),
                |cfg, _| match cfg {
                    ConnectionConfig::MongoDB(c) => Some(mongosh_command(c, true)),
                    _ => None,
                },
            )),
    }
}

fn structure_supported(engine: EngineKind, kind: ObjectKind) -> bool {
    match engine {
        EngineKind::Postgres => matches!(
            kind,
            ObjectKind::Table | ObjectKind::View | ObjectKind::MaterializedView
        ),
        EngineKind::SQLite => matches!(kind, ObjectKind::Table | ObjectKind::View),
        EngineKind::MongoDB => false,
    }
}

fn connection_idx(tree: &ConnectionTree, conn_id: &ConnectionId, cx: &App) -> Option<usize> {
    tree.registry
        .read(cx)
        .connections()
        .iter()
        .position(|e| e.read(cx).id == *conn_id)
}

/// Shared New Query / Refresh / copy / Disconnect items.
///
/// `include_new_window` is for the tree context menu only; the status-bar chip skips it.
pub(crate) fn connection_actions_menu(
    conn_id: ConnectionId,
    engine: EngineKind,
    is_connected: bool,
    tree: WeakEntity<ConnectionTree>,
    menu: PopupMenu,
    include_new_window: bool,
) -> PopupMenu {
    let tree_menu = tree.clone();
    let mut menu = menu.item(PopupMenuItem::new("New Query").on_click({
        let tree = tree_menu.clone();
        let conn_id = conn_id.clone();
        move |_, _, cx| {
            if let Some(tree_ent) = tree.upgrade() {
                tree_ent.update(cx, |tree, cx| {
                    if let Some(idx) = connection_idx(tree, &conn_id, cx) {
                        tree.open_new_query(idx, cx);
                    }
                });
            }
        }
    }));

    if is_connected {
        menu = menu.item(PopupMenuItem::new("Refresh").on_click({
            let tree = tree_menu.clone();
            let conn_id = conn_id.clone();
            move |_, _, cx| {
                if let Some(tree_ent) = tree.upgrade() {
                    tree_ent.update(cx, |tree, cx| {
                        if let Some(idx) = connection_idx(tree, &conn_id, cx) {
                            tree.refresh_connection(idx, cx);
                        }
                    });
                }
            }
        }));
    }

    menu = add_copy_items(menu, tree_menu.clone(), conn_id.clone(), engine);

    if include_new_window {
        menu = menu
            .separator()
            .item(coming_soon_item("Open in New Window"))
            .separator();
    } else {
        menu = menu.separator();
    }

    if is_connected {
        menu = menu.item(PopupMenuItem::new("Disconnect").on_click({
            let tree = tree_menu;
            move |_, _, cx| {
                if let Some(tree_ent) = tree.upgrade() {
                    tree_ent.update(cx, |tree, cx| {
                        if let Some(idx) = connection_idx(tree, &conn_id, cx) {
                            tree.disconnect_at(idx, cx);
                        }
                    });
                }
            }
        }));
    }

    menu
}

pub(crate) fn connection_context_menu(
    idx: usize,
    engine: EngineKind,
    is_connected: bool,
    tree: WeakEntity<ConnectionTree>,
    menu: PopupMenu,
    cx: &mut App,
) -> PopupMenu {
    if let Some(tree_ent) = tree.upgrade() {
        tree_ent.update(cx, |tree, cx| {
            tree.selected_connection = Some(idx);
            cx.notify();
        });
    }

    let Some(conn_id) = tree.upgrade().and_then(|tree_ent| {
        tree_ent
            .read(cx)
            .registry
            .read(cx)
            .connections()
            .get(idx)
            .map(|entry| entry.read(cx).id.clone())
    }) else {
        return menu;
    };

    connection_actions_menu(conn_id, engine, is_connected, tree, menu, true)
}

/// Connection-wide refresh; per-schema invalidation is future work.
pub(crate) fn schema_context_menu(
    conn_idx: usize,
    schema_name: String,
    expanded: bool,
    is_connected: bool,
    tree: WeakEntity<ConnectionTree>,
    menu: PopupMenu,
    cx: &mut App,
) -> PopupMenu {
    if let Some(tree_ent) = tree.upgrade() {
        tree_ent.update(cx, |tree, cx| {
            tree.selected_connection = Some(conn_idx);
            cx.notify();
        });
    }

    let tree_menu = tree.clone();
    let mut menu = menu;

    if is_connected {
        menu = menu.item(PopupMenuItem::new("Refresh").on_click({
            let tree = tree_menu.clone();
            move |_, _, cx| {
                if let Some(tree_ent) = tree.upgrade() {
                    tree_ent.update(cx, |tree, cx| tree.refresh_connection(conn_idx, cx));
                }
            }
        }));
    }

    let toggle_label = if expanded { "Collapse" } else { "Expand" };
    menu.item(PopupMenuItem::new(toggle_label).on_click({
        let tree = tree_menu;
        move |_, _, cx| {
            if let Some(tree_ent) = tree.upgrade() {
                tree_ent.update(cx, |t, cx| {
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
        }
    }))
}

pub(crate) fn object_context_menu(
    conn_idx: usize,
    object: SchemaObject,
    engine: EngineKind,
    is_connected: bool,
    tree: WeakEntity<ConnectionTree>,
    menu: PopupMenu,
    cx: &mut App,
) -> PopupMenu {
    let object_name = object.display_name();
    if let Some(tree_ent) = tree.upgrade() {
        tree_ent.update(cx, |tree, cx| {
            tree.selected_connection = Some(conn_idx);
            tree.selected_object = Some(object_name.clone());
            cx.notify();
        });
    }

    let tree_menu = tree.clone();
    let object_data = object.clone();
    let mut menu = menu.item(PopupMenuItem::new("Open Data").on_click({
        let tree = tree_menu.clone();
        let object = object_data.clone();
        move |_, _, cx| {
            if let Some(tree_ent) = tree.upgrade() {
                tree_ent.update(cx, |tree, cx| {
                    tree.open_data_tab(object.clone(), conn_idx, cx)
                });
            }
        }
    }));

    if structure_supported(engine, object.kind.clone()) {
        let object_struct = object.clone();
        menu = menu.item(PopupMenuItem::new("Open Structure").on_click({
            let tree = tree_menu.clone();
            move |_, _, cx| {
                if let Some(tree_ent) = tree.upgrade() {
                    tree_ent.update(cx, |tree, cx| {
                        tree.open_structure_tab(object_struct.clone(), conn_idx, cx);
                    });
                }
            }
        }));
    }

    let copy_name = object.display_name();
    menu = menu
        .separator()
        .item(PopupMenuItem::new("Copy Name").on_click(move |_, _, cx| {
            cx.write_to_clipboard(ClipboardItem::new_string(copy_name.clone()));
        }))
        .separator();

    if is_connected {
        menu = menu
            .item(coming_soon_item("Export Data…"))
            .item(coming_soon_item("Open in New Window"))
            .separator()
            .item(coming_soon_item("New Query…"));
    }

    menu
}

/// Whether the connection at `conn_idx` is in the Connected state.
pub(crate) fn connection_is_connected(tree: &ConnectionTree, conn_idx: usize, cx: &App) -> bool {
    tree.registry
        .read(cx)
        .connections()
        .get(conn_idx)
        .is_some_and(|e| matches!(e.read(cx).state, ConnectionState::Connected(_)))
}
