//! SQLite query round trip: real connection, real editor panel, real results grid.

use based_desktop::connection::ConnectionId;
use based_desktop::workspace::panels::query_editor::QueryEditorPanel;
use based_desktop::workspace::panels::query_view::ResultSnapshot;
use based_desktop::workspace::{QueryEditorInit, TabSpec, Workspace};
use gpui_kit::test::TestWindowExt;
use gpui_kit::{AnyWindowHandle, AppContext, Entity, TestAppContext};

use crate::common::{connect_sqlite, open_workspace, seed_sqlite, settle, wait_until};

const SEED: &[&str] = &[
    "create table t (id integer primary key, name text)",
    "insert into t values (1, 'ada'), (2, 'grace')",
];

struct Editor {
    win: AnyWindowHandle,
    panel: Entity<QueryEditorPanel>,
}

impl Editor {
    fn snapshot(&self, cx: &mut TestAppContext) -> ResultSnapshot {
        self.panel.read_with(cx, |p, cx| p.result_snapshot(cx))
    }

    /// Wait for a finished run (rows or an error).
    fn finished(&self, cx: &mut TestAppContext) -> ResultSnapshot {
        wait_until(cx, |cx| {
            let s = self.snapshot(cx);
            !s.running && (!s.rows.is_empty() || s.error.is_some())
        });
        self.snapshot(cx)
    }
}

/// Seed a SQLite file, connect it, and open a query tab with `sql`.
fn open_editor(
    cx: &mut TestAppContext,
    sql: &str,
    auto_run: bool,
) -> (Editor, Entity<Workspace>, tempfile::TempDir) {
    let dir = tempfile::tempdir().unwrap();
    let db = dir.path().join("demo.db");
    seed_sqlite(&db, SEED);
    let (win, ws) = open_workspace(cx);
    let conn = connect_sqlite(cx, &ws, "demo", &db);
    open_query_tab(cx, win, &ws, conn, sql, auto_run);
    let panel = ws.read_with(cx, |ws, cx| {
        ws.tab_manager()
            .read(cx)
            .tabs
            .iter()
            .find_map(|t| t.view.clone().downcast::<QueryEditorPanel>().ok())
            .expect("query editor tab")
    });
    (Editor { win, panel }, ws, dir)
}

fn open_query_tab(
    cx: &mut TestAppContext,
    win: AnyWindowHandle,
    ws: &Entity<Workspace>,
    conn_id: ConnectionId,
    sql: &str,
    auto_run: bool,
) {
    let spec = TabSpec::QueryEditor {
        conn_id,
        init: QueryEditorInit::Sql {
            sql: Some(sql.into()),
            auto_run,
        },
    };
    cx.update_window(win, |_, window, cx| {
        let opened = ws.update(cx, |ws, cx| ws.dispatch_open_tab(spec, window, cx));
        assert!(opened, "query tab refused (connection not live?)");
        window.render_frame(cx);
    })
    .unwrap();
    settle(cx);
}

#[gpui_kit::test]
fn auto_run_shows_rows(cx: &mut TestAppContext) {
    let (ed, _ws, _dir) = open_editor(cx, "select id, name from t order by id", true);
    let result = ed.finished(cx);
    assert_eq!(result.columns, ["id", "name"]);
    assert_eq!(result.rows, [["1", "ada"], ["2", "grace"]]);
    assert_eq!(result.error, None);
}

#[gpui_kit::test]
fn run_button_executes_the_query(cx: &mut TestAppContext) {
    let (ed, _ws, _dir) = open_editor(cx, "select name from t where id = 2", false);
    assert_eq!(
        ed.snapshot(cx),
        ResultSnapshot::default(),
        "nothing runs yet"
    );
    cx.update_window(ed.win, |_, window, cx| {
        window.render_frame(cx);
        window.click("sqlite-run", cx);
    })
    .unwrap();
    assert_eq!(ed.finished(cx).rows, [["grace"]]);
}

#[gpui_kit::test]
fn invalid_sql_reports_an_error(cx: &mut TestAppContext) {
    let (ed, _ws, _dir) = open_editor(cx, "selec nope", true);
    let result = ed.finished(cx);
    assert!(result.error.is_some(), "{result:?}");
    assert!(result.rows.is_empty());
}
