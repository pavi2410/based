//! Right-click context menus for the connection browser tree.

use gpui_kit::component::menu::{PopupMenu, PopupMenuItem};
use gpui_kit::component::{Icon, IconName};
use gpui_kit::{App, ClipboardItem, WeakEntity};

use crate::connection::{ConnectionConfig, ConnectionId, ConnectionState, EngineKind};
use crate::mongodb::{mongo_uri, mongosh_command};
use crate::postgres::{postgres_uri, psql_command};
use crate::redis::{redis_cli, redis_uri};
use crate::sqlite::{resolve_sqlite_path, sqlite_uri, sqlite3_command};
use crate::workspace::TabSpec;
use crate::workspace::tabs::WorkspaceRef;
use crate::workspace::tabs::{inbox, request_workspace_flush};
use crate::workspace::wizard_logic::can_edit_saved_connection;

use crate::ai::ask_assistant;

use super::ConnectionTree;
use super::types::{DbObject, ObjectKind};

const ICON_PENCIL: &str = "icons/pencil.svg";
const ICON_COPY: &str = "icons/copy.svg";
const ICON_KEY: &str = "icons/key-round.svg";
const ICON_TERMINAL: &str = "icons/square-terminal.svg";
const ICON_UNPLUG: &str = "icons/unplug.svg";
const ICON_SPARKLES: &str = "icons/sparkles.svg";
const ICON_TABLE: &str = "icons/table-2.svg";
const ICON_COLUMNS: &str = "icons/columns-3.svg";

fn svg(path: &'static str) -> Icon {
    Icon::empty().path(path)
}

fn copy_config_item(
    label: &'static str,
    tree: WeakEntity<ConnectionTree>,
    conn_id: ConnectionId,
    text: impl Fn(&ConnectionConfig, &App) -> Option<String> + 'static,
) -> PopupMenuItem {
    let icon = if label.contains("password") {
        svg(ICON_KEY)
    } else if label.contains("Command") {
        svg(ICON_TERMINAL)
    } else {
        svg(ICON_COPY)
    };
    PopupMenuItem::new(label)
        .icon(icon)
        .on_click(move |_, _, cx| {
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
                "Copy Connection String (includes password)",
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
                "Copy psql Command (includes password)",
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
                "Copy Connection String (includes password)",
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
                "Copy mongosh Command (includes password)",
                tree,
                conn_id.clone(),
                |cfg, _| match cfg {
                    ConnectionConfig::MongoDB(c) => Some(mongosh_command(c, true)),
                    _ => None,
                },
            )),
        EngineKind::Redis => menu
            .item(copy_config_item(
                "Copy Connection URL",
                tree.clone(),
                conn_id.clone(),
                |cfg, _| match cfg {
                    ConnectionConfig::Redis(c) => Some(redis_uri(c, false)),
                    _ => None,
                },
            ))
            .item(copy_config_item(
                "Copy Connection URL (includes password)",
                tree.clone(),
                conn_id.clone(),
                |cfg, _| match cfg {
                    ConnectionConfig::Redis(c) => Some(redis_uri(c, true)),
                    _ => None,
                },
            ))
            .item(copy_config_item(
                "Copy redis-cli Command",
                tree.clone(),
                conn_id.clone(),
                |cfg, _| match cfg {
                    ConnectionConfig::Redis(c) => Some(redis_cli(c, false)),
                    _ => None,
                },
            ))
            .item(copy_config_item(
                "Copy redis-cli Command (includes password)",
                tree,
                conn_id.clone(),
                |cfg, _| match cfg {
                    ConnectionConfig::Redis(c) => Some(redis_cli(c, true)),
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
        EngineKind::MongoDB | EngineKind::Redis => false,
    }
}

fn conn_id_at(tree: &WeakEntity<ConnectionTree>, idx: usize, cx: &App) -> Option<ConnectionId> {
    let tree = tree.upgrade()?;
    let registry = tree.read(cx).registry.read(cx);
    registry
        .connections()
        .get(idx)
        .map(|e| e.read(cx).id.clone())
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
    let mut menu = menu.item(
        PopupMenuItem::new("New Query")
            .icon(svg(ICON_TERMINAL))
            .on_click({
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
            }),
    );

    if is_connected {
        let overview_id = conn_id.clone();
        menu = menu.item(
            PopupMenuItem::new("Overview")
                .icon(IconName::Info)
                .on_click(move |_, _, cx| {
                    inbox::open_tab(
                        TabSpec::Dashboard {
                            conn_id: overview_id.clone(),
                        },
                        cx,
                    );
                    request_workspace_flush(cx);
                }),
        );
    }

    if can_edit_saved_connection(&conn_id) {
        let edit_id = conn_id.clone();
        menu = menu.item(PopupMenuItem::new("Edit…").icon(svg(ICON_PENCIL)).on_click(
            move |_, window, cx| {
                if let Some(ws) = cx.try_global::<WorkspaceRef>().map(|w| w.0.clone()) {
                    ws.update(cx, |ws, cx| {
                        ws.open_edit_connection_tab(edit_id.clone(), window, cx);
                    });
                }
            },
        ));
    }

    if is_connected {
        menu = menu.item(
            PopupMenuItem::new("Refresh")
                .icon(IconName::RefreshCw)
                .on_click({
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
                }),
        );
    }

    menu = add_copy_items(
        menu.separator().label("Copy"),
        tree_menu.clone(),
        conn_id.clone(),
        engine,
    );

    // Unfinished items stay out of the menu until they work.
    let _ = include_new_window;

    if is_connected {
        menu = menu.separator();
        menu = menu.item(
            PopupMenuItem::new("Disconnect")
                .icon(svg(ICON_UNPLUG))
                .on_click({
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
                }),
        );
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
        menu = menu.item(
            PopupMenuItem::new("Refresh")
                .icon(IconName::RefreshCw)
                .on_click({
                    let tree = tree_menu.clone();
                    move |_, _, cx| {
                        if let Some(tree_ent) = tree.upgrade() {
                            tree_ent.update(cx, |tree, cx| tree.refresh_connection(conn_idx, cx));
                        }
                    }
                }),
        );
    }

    let toggle_label = if expanded { "Collapse" } else { "Expand" };
    menu.item(
        PopupMenuItem::new(toggle_label)
            .icon(if expanded {
                IconName::ChevronDown
            } else {
                IconName::ChevronRight
            })
            .on_click({
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
            }),
    )
}

pub(crate) fn object_context_menu(
    conn_idx: usize,
    object: DbObject,
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
    let mut menu = menu.item(
        PopupMenuItem::new("Open Data")
            .icon(svg(ICON_TABLE))
            .on_click({
                let tree = tree_menu.clone();
                let object = object_data.clone();
                move |_, _, cx| {
                    if let Some(tree_ent) = tree.upgrade() {
                        tree_ent.update(cx, |tree, cx| {
                            tree.open_data_tab(object.clone(), conn_idx, cx)
                        });
                    }
                }
            }),
    );

    if structure_supported(engine, object.kind) {
        let object_struct = object.clone();
        menu = menu.item(
            PopupMenuItem::new("Open Structure")
                .icon(svg(ICON_COLUMNS))
                .on_click({
                    let tree = tree_menu.clone();
                    move |_, _, cx| {
                        if let Some(tree_ent) = tree.upgrade() {
                            tree_ent.update(cx, |tree, cx| {
                                tree.open_structure_tab(object_struct.clone(), conn_idx, cx);
                            });
                        }
                    }
                }),
        );
    }

    if is_connected && let Some(conn_id) = conn_id_at(&tree, conn_idx, cx) {
        let text = format!(
            "Tell me about the {} `{}`: what it holds and how to query it well.",
            object.kind.label(),
            object.display_name()
        );
        menu = menu.item(
            PopupMenuItem::new("Ask AI About This")
                .icon(svg(ICON_SPARKLES))
                .on_click(move |_, _, cx| {
                    ask_assistant(conn_id.clone(), text.clone(), cx);
                }),
        );
    }

    let copy_name = object.display_name();
    menu = menu
        .separator()
        .item(
            PopupMenuItem::new("Copy Name")
                .icon(svg(ICON_COPY))
                .on_click(move |_, _, cx| {
                    cx.write_to_clipboard(ClipboardItem::new_string(copy_name.clone()));
                }),
        )
        .separator();

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
