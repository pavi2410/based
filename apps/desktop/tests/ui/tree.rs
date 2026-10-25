//! Sidebar connection rail and catalog browser.

use based_desktop::connection::ConnectionId;
use based_desktop::workspace::connection_tree::{CatalogSnapshot, ObjectKind};
use based_desktop::workspace::{TabSpec, Workspace};
use gpui_kit::test::TestWindowExt;
use gpui_kit::{AnyWindowHandle, AppContext, Entity, TestAppContext};

use crate::common::{connect_sqlite, open_workspace, seed_sqlite, settle, wait_until};

const SEED: &[&str] = &[
    "create table users (id integer primary key, name text)",
    "create table posts (id integer primary key, user_id integer)",
    "create view user_names as select name from users",
    "create trigger posts_ai after insert on posts begin select 1; end",
];

struct Tree {
    win: AnyWindowHandle,
    ws: Entity<Workspace>,
    conn: ConnectionId,
    _dir: tempfile::TempDir,
}

impl Tree {
    fn catalog(&self, cx: &mut TestAppContext) -> CatalogSnapshot {
        self.ws
            .read_with(cx, |ws, cx| ws.connection_tree().read(cx).catalog(cx))
    }

    fn names(&self, cx: &mut TestAppContext) -> Vec<String> {
        self.catalog(cx)
            .objects
            .iter()
            .map(|o| o.display_name())
            .collect()
    }

    fn render(&self, cx: &mut TestAppContext) {
        cx.update_window(self.win, |_, window, cx| window.render_frame(cx))
            .unwrap();
        settle(cx);
    }

    fn click_object(&self, name: &str, cx: &mut TestAppContext) {
        let tree = self.ws.read_with(cx, |ws, _| ws.connection_tree().clone());
        cx.update_window(self.win, |_, window, cx| {
            assert!(
                tree.update(cx, |t, cx| t.click_object(name, window, cx)),
                "{name}"
            );
            window.render_frame(cx);
        })
        .unwrap();
        settle(cx);
    }

    fn tabs(&self, cx: &mut TestAppContext) -> Vec<TabSpec> {
        self.ws.read_with(cx, |ws, cx| {
            ws.tab_manager()
                .read(cx)
                .tabs
                .iter()
                .map(|t| t.spec.clone())
                .collect()
        })
    }
}

/// A connected SQLite database whose rail icon has been clicked.
fn opened(cx: &mut TestAppContext) -> Tree {
    let dir = tempfile::tempdir().unwrap();
    let db = dir.path().join("demo.db");
    seed_sqlite(&db, SEED);
    let (win, ws) = open_workspace(cx);
    let conn = connect_sqlite(cx, &ws, "demo", &db);
    let tree = Tree {
        win,
        ws,
        conn,
        _dir: dir,
    };
    tree.render(cx);
    let tree_entity = tree.ws.read_with(cx, |ws, _| ws.connection_tree().clone());
    cx.update_window(win, |_, window, cx| {
        tree_entity.update(cx, |t, cx| t.click_connection(0, window, cx));
    })
    .unwrap();
    tree.render(cx);
    wait_until(cx, |cx| {
        tree.render(cx);
        !tree.catalog(cx).objects.is_empty()
    });
    tree
}

#[gpui_kit::test]
fn nothing_selected_without_connections(cx: &mut TestAppContext) {
    let (_win, ws) = open_workspace(cx);
    let snap = ws.read_with(cx, |ws, cx| ws.connection_tree().read(cx).catalog(cx));
    assert_eq!(snap.selected, None);
    assert!(snap.objects.is_empty());
}

#[gpui_kit::test]
fn clicking_a_connection_lists_its_objects(cx: &mut TestAppContext) {
    let tree = opened(cx);
    assert_eq!(tree.catalog(cx).selected, Some(0));
    let mut names = tree.names(cx);
    names.sort();
    assert_eq!(names, ["posts", "posts_ai", "user_names", "users"]);
    let kind = |n: &str, cx: &mut TestAppContext| {
        tree.catalog(cx)
            .objects
            .iter()
            .find(|o| o.name == n)
            .unwrap()
            .kind
    };
    assert_eq!(kind("users", cx), ObjectKind::Table);
    assert_eq!(kind("user_names", cx), ObjectKind::View);
    assert_eq!(kind("posts_ai", cx), ObjectKind::Trigger);
}

#[gpui_kit::test]
fn clicking_a_table_opens_its_data_viewer(cx: &mut TestAppContext) {
    let tree = opened(cx);
    tree.click_object("users", cx);
    assert!(tree.tabs(cx).contains(&TabSpec::DataViewer {
        conn_id: tree.conn.clone(),
        object: "users".into(),
        filter: None,
    }));
}

#[gpui_kit::test]
fn clicking_a_trigger_opens_object_info(cx: &mut TestAppContext) {
    let tree = opened(cx);
    tree.click_object("posts_ai", cx);
    assert!(tree.tabs(cx).iter().any(
        |t| matches!(t, TabSpec::ObjectInfo { object_name, .. } if object_name == "posts_ai")
    ));
}

#[gpui_kit::test]
fn clicking_an_object_twice_keeps_one_tab(cx: &mut TestAppContext) {
    let tree = opened(cx);
    tree.click_object("users", cx);
    let n = tree.tabs(cx).len();
    tree.click_object("users", cx);
    assert_eq!(tree.tabs(cx).len(), n);
}

#[gpui_kit::test]
fn catalog_search_filters_and_clears(cx: &mut TestAppContext) {
    let tree = opened(cx);
    let tree_entity = tree.ws.read_with(cx, |ws, _| ws.connection_tree().clone());
    tree_entity.update(cx, |t, cx| t.search_catalog("post", cx));
    tree.render(cx);
    let mut names = tree.names(cx);
    names.sort();
    assert_eq!(names, ["posts", "posts_ai"]);
    tree_entity.update(cx, |t, cx| t.search_catalog("", cx));
    tree.render(cx);
    assert_eq!(tree.names(cx).len(), 4);
}
