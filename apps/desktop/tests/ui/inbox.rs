//! Deferred workspace requests: ordering and the SQL-injection flows.

use based_desktop::connection::ConnectionId;
use based_desktop::workspace::panels::query_editor::QueryEditorPanel;
use based_desktop::workspace::tabs::inbox;
use based_desktop::workspace::{QueryEditorInit, TabSpec, Workspace};
use gpui_kit::test::TestWindowExt;
use gpui_kit::{AnyWindowHandle, AppContext, Entity, TestAppContext};

use crate::common::{connect_sqlite, info, open_workspace, seed_sqlite, settle};

struct Setup {
    win: AnyWindowHandle,
    ws: Entity<Workspace>,
    conn: ConnectionId,
    _dir: tempfile::TempDir,
}

fn setup(cx: &mut TestAppContext) -> Setup {
    let dir = tempfile::tempdir().unwrap();
    let db = dir.path().join("demo.db");
    seed_sqlite(&db, &["create table t (id integer)"]);
    let (win, ws) = open_workspace(cx);
    let conn = connect_sqlite(cx, &ws, "demo", &db);
    Setup {
        win,
        ws,
        conn,
        _dir: dir,
    }
}

impl Setup {
    fn render(&self, cx: &mut TestAppContext) {
        cx.update_window(self.win, |_, window, cx| window.render_frame(cx))
            .unwrap();
        settle(cx);
    }

    fn specs(&self, cx: &mut TestAppContext) -> Vec<TabSpec> {
        self.ws.read_with(cx, |ws, cx| {
            ws.tab_manager()
                .read(cx)
                .tabs
                .iter()
                .map(|t| t.spec.clone())
                .collect()
        })
    }

    fn editor(&self, cx: &mut TestAppContext) -> Option<Entity<QueryEditorPanel>> {
        self.ws.read_with(cx, |ws, cx| {
            ws.tab_manager()
                .read(cx)
                .active_tab()
                .and_then(|t| t.downcast())
        })
    }
}

#[gpui_kit::test]
fn requests_apply_in_order(cx: &mut TestAppContext) {
    let s = setup(cx);
    cx.update(|cx| {
        inbox::open_tab(info("a"), cx);
        inbox::open_tab(info("b"), cx);
        inbox::open_tab(info("c"), cx); // a single-slot queue would keep only this one
    });
    s.render(cx);
    let specs = s.specs(cx);
    assert_eq!(&specs[specs.len() - 3..], [info("a"), info("b"), info("c")]);
}

#[gpui_kit::test]
fn injected_sql_replaces_the_active_editor_text(cx: &mut TestAppContext) {
    let s = setup(cx);
    cx.update(|cx| inbox::open_tab(TabSpec::blank_query_editor(s.conn.clone()), cx));
    s.render(cx);
    let editor = s.editor(cx).expect("active query editor");
    cx.update(|cx| inbox::inject_sql(s.conn.clone(), "select 42".into(), cx));
    s.render(cx);
    assert_eq!(editor.read_with(cx, |e, cx| e.current_sql(cx)), "select 42");
    assert!(
        s.ws.read_with(cx, |ws, cx| ws.has_dirty_tabs(cx)),
        "injected text is an edit"
    );
}

#[gpui_kit::test]
fn injecting_without_an_editor_opens_one(cx: &mut TestAppContext) {
    let s = setup(cx);
    cx.update(|cx| inbox::inject_sql(s.conn.clone(), "select 7".into(), cx));
    s.render(cx);
    let opened = s.specs(cx).into_iter().any(|t| {
        matches!(t, TabSpec::QueryEditor { init: QueryEditorInit::Sql { sql: Some(ref q), .. }, .. } if q == "select 7")
    });
    assert!(opened, "{:?}", s.specs(cx));
}
