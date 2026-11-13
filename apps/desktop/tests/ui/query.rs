//! SQLite query round trip: real connection, real editor panel, real results grid.

use based_desktop::connection::ConnectionId;
use based_desktop::query_store::QueryStore;
use based_desktop::workspace::panels::query_editor::{HoldReason, QueryEditorPanel};
use based_desktop::workspace::panels::query_view::ResultSnapshot;
use based_desktop::workspace::tabs::spec::SavedQueryRef;
use based_desktop::workspace::tabs::{inbox, request_workspace_flush};
use based_desktop::workspace::{QueryEditorInit, TabSpec, Workspace};
use based_query::RunOutcome;
use gpui_kit::test::TestWindowExt;
use gpui_kit::{AnyWindowHandle, AppContext, Entity, TestAppContext};

use crate::common::{
    connect_sqlite, connect_sqlite_tagged, open_workspace, seed_sqlite, settle, wait_until,
};

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

    fn hold(&self, cx: &mut TestAppContext) -> Option<HoldReason> {
        self.panel.read_with(cx, |p, _| p.hold_reason())
    }

    /// Render, then click the element with `id` (a run or banner button).
    fn click(&self, id: &'static str, cx: &mut TestAppContext) {
        cx.update_window(self.win, |_, window, cx| {
            window.render_frame(cx);
            window.click(id, cx);
        })
        .unwrap();
        settle(cx);
    }

    fn set_sql(&self, sql: &str, cx: &mut TestAppContext) {
        cx.update_window(self.win, |_, window, cx| {
            self.panel.update(cx, |p, cx| p.set_sql(sql, window, cx));
        })
        .unwrap();
        settle(cx);
    }

    fn apply_ai(&self, sql: &str, cx: &mut TestAppContext) {
        cx.update_window(self.win, |_, window, cx| {
            self.panel
                .update(cx, |p, cx| p.apply_ai_proposal(sql, window, cx));
        })
        .unwrap();
        settle(cx);
    }

    /// Rows left in the seeded table, counted through the editor.
    fn row_count(&self, cx: &mut TestAppContext) -> String {
        self.set_sql("select count(*) as n from t", cx);
        self.click("sqlite-run", cx);
        wait_until(cx, |cx| {
            let s = self.snapshot(cx);
            !s.running && s.columns == ["n"]
        });
        self.snapshot(cx).rows[0][0].clone()
    }
}

/// Seed a SQLite file, connect it, and open a query tab with `sql`.
fn open_editor(
    cx: &mut TestAppContext,
    sql: &str,
    auto_run: bool,
) -> (Editor, Entity<Workspace>, tempfile::TempDir) {
    open_editor_tagged(cx, sql, auto_run, &[])
}

/// [`open_editor`] on a connection carrying `tags` (e.g. `"production"`).
fn open_editor_tagged(
    cx: &mut TestAppContext,
    sql: &str,
    auto_run: bool,
    tags: &[&str],
) -> (Editor, Entity<Workspace>, tempfile::TempDir) {
    let dir = tempfile::tempdir().unwrap();
    let db = dir.path().join("demo.db");
    seed_sqlite(&db, SEED);
    let (win, ws) = open_workspace(cx);
    let tags = tags.iter().map(|t| t.to_string()).collect();
    let conn = connect_sqlite_tagged(cx, &ws, "demo", &db, tags);
    open_query_tab(cx, win, &ws, conn, sql, auto_run);
    let panel = ws.read_with(cx, |ws, cx| {
        ws.tab_manager()
            .read(cx)
            .tabs
            .iter()
            .find_map(|t| t.downcast::<QueryEditorPanel>())
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
        init: QueryEditorInit {
            sql: Some(sql.into()),
            auto_run,
            saved: None,
            dirty: false,
        },
    };
    cx.update_window(win, |_, window, cx| {
        let opened = ws.update(cx, |ws, cx| ws.dispatch_open_tab(spec, window, cx));
        assert!(opened.is_open(), "query tab refused (connection not live?)");
        window.render_frame(cx);
    })
    .unwrap();
    settle(cx);
}

#[gpui_kit::test]
fn stopping_a_run_leaves_nothing_running(cx: &mut TestAppContext) {
    // Slow enough to still be running when Stop is clicked.
    let slow = "with recursive c(x) as (select 1 union all select x + 1 from c limit 50000000) \
                select count(*) from c";
    let (ed, _ws, _dir) = open_editor(cx, slow, false);
    ed.click("sqlite-run", cx);
    assert!(ed.snapshot(cx).running, "run started");
    ed.click("sqlite-stop", cx);
    let s = ed.snapshot(cx);
    assert!(!s.running, "stopped");
    assert!(
        s.rows.is_empty() && s.error.is_none(),
        "no result, no error"
    );
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
    // History records why it failed.
    let outcome = cx.update(|cx| {
        cx.global::<QueryStore>()
            .history
            .recent(1)
            .first()
            .map(|h| h.outcome.clone())
    });
    assert!(
        matches!(&outcome, Some(RunOutcome::Error { summary }) if !summary.is_empty()),
        "{outcome:?}"
    );
}

#[gpui_kit::test]
fn run_with_several_statements_runs_the_one_at_the_cursor(cx: &mut TestAppContext) {
    let (ed, _ws, _dir) = open_editor(
        cx,
        "select name from t where id = 1;\nselect name from t where id = 2",
        false,
    );
    cx.update_window(ed.win, |_, window, cx| {
        window.render_frame(cx);
        window.click("sqlite-run", cx);
    })
    .unwrap();
    // The cursor starts in the first statement, so only it runs.
    assert_eq!(ed.finished(cx).rows, [["ada"]]);
}

#[gpui_kit::test]
fn run_all_gives_each_statement_its_own_result(cx: &mut TestAppContext) {
    let (ed, _ws, _dir) = open_editor(
        cx,
        "select name from t where id = 1;\nselect name from t where id = 2",
        false,
    );
    ed.panel.update(cx, |p, cx| p.run_all(cx));
    wait_until(cx, |cx| {
        let s = ed.snapshot(cx);
        !s.running && s.result_count == 2
    });
    let last = ed.snapshot(cx);
    assert_eq!(last.rows, [["grace"]], "the last result is shown first");
    assert_eq!(last.active_result, 1);

    ed.panel.update(cx, |p, cx| p.show_result(0, cx));
    assert_eq!(ed.snapshot(cx).rows, [["ada"]]);
}

#[gpui_kit::test]
fn run_all_stops_at_the_first_failure(cx: &mut TestAppContext) {
    let (ed, _ws, _dir) = open_editor(
        cx,
        "select name from t where id = 1;\nselec nope;\nselect name from t where id = 2",
        false,
    );
    ed.panel.update(cx, |p, cx| p.run_all(cx));
    wait_until(cx, |cx| {
        let s = ed.snapshot(cx);
        !s.running && s.error.is_some()
    });
    let s = ed.snapshot(cx);
    assert_eq!(
        s.result_count, 1,
        "only the statement before the failure produced a result"
    );
}

#[gpui_kit::test]
fn the_error_highlight_is_in_the_statement_that_failed(cx: &mut TestAppContext) {
    // `selec` also appears, quoted, in the first statement; only the second fails.
    let sql = "select 'selec';\nselec nope";
    let (ed, _ws, _dir) = open_editor(cx, sql, false);
    ed.panel.update(cx, |p, cx| p.run_all(cx));
    wait_until(cx, |cx| {
        let s = ed.snapshot(cx);
        !s.running && s.error.is_some()
    });
    let span = ed.snapshot(cx).error_span.expect("a highlight");
    assert_eq!(span, 16..21);
    assert_eq!(&sql[span], "selec");
}

#[gpui_kit::test]
fn replacing_the_query_is_undoable_and_marks_the_tab_dirty(cx: &mut TestAppContext) {
    let (ed, ws, _dir) = open_editor(cx, "select 1", false);
    assert!(
        !ws.read_with(cx, |ws, cx| ws.has_dirty_tabs(cx)),
        "fresh tab is clean"
    );

    cx.update_window(ed.win, |_, window, cx| {
        ed.panel.update(cx, |p, cx| {
            p.set_sql("select 2", window, cx);
            p.focus_editor(window, cx);
        });
        window.render_frame(cx);
    })
    .unwrap();
    settle(cx);
    assert_eq!(
        ed.panel.read_with(cx, |p, cx| p.current_sql(cx)),
        "select 2"
    );
    assert!(ws.read_with(cx, |ws, cx| ws.has_dirty_tabs(cx)));

    let undo = if cfg!(target_os = "macos") {
        "cmd-z"
    } else {
        "ctrl-z"
    };
    cx.simulate_keystrokes(ed.win, undo);
    settle(cx);
    assert_eq!(
        ed.panel.read_with(cx, |p, cx| p.current_sql(cx)),
        "select 1"
    );
}

#[gpui_kit::test]
fn restored_tab_keeps_its_saved_query_and_unsaved_state(cx: &mut TestAppContext) {
    let dir = tempfile::tempdir().unwrap();
    let db = dir.path().join("demo.db");
    seed_sqlite(&db, SEED);
    let (win, ws) = open_workspace(cx);
    let conn_id = connect_sqlite(cx, &ws, "demo", &db);
    let saved = SavedQueryRef {
        path: "reports/top-users".into(),
        name: "Top users".into(),
    };
    let spec = TabSpec::QueryEditor {
        conn_id,
        init: QueryEditorInit {
            sql: Some("select 1".into()),
            auto_run: false,
            saved: Some(saved.clone()),
            dirty: true,
        },
    };
    cx.update_window(win, |_, window, cx| {
        assert!(
            ws.update(cx, |ws, cx| ws.dispatch_open_tab(spec, window, cx))
                .is_open()
        );
        window.render_frame(cx);
    })
    .unwrap();
    settle(cx);

    let panel = ws.read_with(cx, |ws, cx| {
        ws.tab_manager()
            .read(cx)
            .tabs
            .iter()
            .find_map(|t| t.downcast::<QueryEditorPanel>())
            .expect("query editor tab")
    });
    assert!(
        ws.read_with(cx, |ws, cx| ws.has_dirty_tabs(cx)),
        "close must still ask"
    );
    // The saved name doubles as the tab title and as the name Save overwrites.
    let (linked, dirty) = panel.read_with(cx, |p, _| (p.saved_query(), p.is_dirty()));
    assert_eq!(linked, Some(saved));
    assert!(dirty);
}

#[gpui_kit::test]
fn production_write_waits_for_its_banner_and_runs_once_confirmed(cx: &mut TestAppContext) {
    let (ed, _ws, _dir) =
        open_editor_tagged(cx, "delete from t where id = 1", false, &["production"]);
    ed.click("sqlite-run", cx);
    assert_eq!(ed.hold(cx), Some(HoldReason::ProductionWrite));

    ed.click("sqlite-write-run", cx);
    wait_until(cx, |cx| !ed.snapshot(cx).running);
    assert_eq!(ed.hold(cx), None);
    assert_eq!(ed.row_count(cx), "1");
}

#[gpui_kit::test]
fn cancelling_the_production_banner_runs_nothing(cx: &mut TestAppContext) {
    let (ed, _ws, _dir) = open_editor_tagged(cx, "delete from t", false, &["production"]);
    ed.click("sqlite-run", cx);
    assert_eq!(ed.hold(cx), Some(HoldReason::ProductionWrite));

    ed.click("sqlite-write-cancel", cx);
    assert_eq!(ed.hold(cx), None);
    assert_eq!(ed.row_count(cx), "2");
}

#[gpui_kit::test]
fn editing_held_text_drops_the_banner(cx: &mut TestAppContext) {
    let (ed, _ws, _dir) = open_editor_tagged(cx, "delete from t", false, &["production"]);
    ed.click("sqlite-run", cx);
    assert_eq!(ed.hold(cx), Some(HoldReason::ProductionWrite));

    ed.set_sql("delete from t where id = 1", cx);
    assert_eq!(ed.hold(cx), None, "the banner described text that is gone");
    assert_eq!(ed.row_count(cx), "2");
}

#[gpui_kit::test]
fn edited_ai_write_runs_as_edited_not_as_proposed(cx: &mut TestAppContext) {
    let (ed, _ws, _dir) = open_editor(cx, "select 1", false);
    ed.apply_ai("delete from t", cx);
    ed.click("sqlite-run", cx);
    assert_eq!(ed.hold(cx), Some(HoldReason::AiWrite));

    // The user narrows the AI's delete; the old banner must not run the original.
    ed.set_sql("delete from t where id = 2", cx);
    assert_eq!(ed.hold(cx), None);
    ed.click("sqlite-run", cx);
    wait_until(cx, |cx| !ed.snapshot(cx).running);
    assert_eq!(
        ed.hold(cx),
        None,
        "edited text is the user's and runs normally"
    );
    assert_eq!(ed.row_count(cx), "1");
}

#[gpui_kit::test]
fn ai_write_on_production_asks_on_both_banners(cx: &mut TestAppContext) {
    let (ed, _ws, _dir) = open_editor_tagged(cx, "select 1", false, &["production"]);
    ed.apply_ai("delete from t where id = 1", cx);
    ed.click("sqlite-run", cx);
    assert_eq!(ed.hold(cx), Some(HoldReason::AiWrite));

    ed.click("sqlite-ai-run", cx);
    assert_eq!(
        ed.hold(cx),
        Some(HoldReason::ProductionWrite),
        "accepting the AI write still asks about production"
    );

    ed.click("sqlite-write-run", cx);
    wait_until(cx, |cx| !ed.snapshot(cx).running);
    assert_eq!(ed.hold(cx), None);
    assert_eq!(ed.row_count(cx), "1");
}

/// An assistant card's SQL, as its Run (`run`) or "Use in editor" button sends it.
fn send_card_sql(cx: &mut TestAppContext, sql: &str, run: bool) {
    cx.update(|cx| {
        inbox::ai_sql(ConnectionId::parse("demo"), sql.into(), run, cx);
        request_workspace_flush(cx);
    });
    settle(cx);
}

fn editors(ws: &Entity<Workspace>, cx: &mut TestAppContext) -> Vec<Entity<QueryEditorPanel>> {
    ws.read_with(cx, |ws, cx| {
        ws.tab_manager()
            .read(cx)
            .tabs
            .iter()
            .filter_map(|t| t.downcast::<QueryEditorPanel>())
            .collect()
    })
}

#[gpui_kit::test]
fn card_sql_used_in_the_editor_asks_before_writing(cx: &mut TestAppContext) {
    let (editor, _ws, _dir) = open_editor(cx, "select 1", false);
    send_card_sql(cx, "delete from t", false);
    editor.click("sqlite-run", cx);
    assert_eq!(editor.hold(cx), Some(HoldReason::AiWrite));
    assert_eq!(editor.row_count(cx), "2", "nothing ran before confirming");
}

#[gpui_kit::test]
fn card_run_of_a_write_waits_for_confirmation(cx: &mut TestAppContext) {
    let (editor, ws, _dir) = open_editor(cx, "select 1", false);
    // Cards only offer Run on text that looks read-only; a mislabelled write still waits.
    send_card_sql(cx, "delete from t", true);
    let run = editors(&ws, cx).pop().expect("a tab for the card's run");
    let card = Editor {
        win: editor.win,
        panel: run,
    };
    assert_eq!(
        card.hold(cx),
        Some(HoldReason::AiWrite),
        "held, so nothing ran"
    );
}

#[gpui_kit::test]
fn card_run_shows_rows(cx: &mut TestAppContext) {
    let (editor, ws, _dir) = open_editor(cx, "select 1", false);
    send_card_sql(cx, "select name from t order by id", true);
    let run = editors(&ws, cx).pop().expect("a tab for the card's run");
    let card = Editor {
        win: editor.win,
        panel: run,
    };
    let s = card.finished(cx);
    assert_eq!(s.rows, [["ada"], ["grace"]]);
}

#[gpui_kit::test]
fn delete_without_where_asks_on_any_connection(cx: &mut TestAppContext) {
    let (editor, _ws, _dir) = open_editor(cx, "delete from t", false);
    editor.click("sqlite-run", cx);
    assert_eq!(editor.hold(cx), Some(HoldReason::Destructive));
    editor.click("sqlite-write-run", cx);
    wait_until(cx, |cx| {
        !editor.snapshot(cx).running && editor.hold(cx).is_none()
    });
    assert_eq!(editor.row_count(cx), "0", "confirmed, so it ran once");
}

#[gpui_kit::test]
fn a_bounded_delete_runs_without_asking(cx: &mut TestAppContext) {
    let (editor, _ws, _dir) = open_editor(cx, "delete from t where id = 1", false);
    editor.click("sqlite-run", cx);
    assert_eq!(editor.hold(cx), None);
    wait_until(cx, |cx| !editor.snapshot(cx).running);
    assert_eq!(editor.row_count(cx), "1");
}

#[gpui_kit::test]
fn a_new_query_tab_runs_nothing(cx: &mut TestAppContext) {
    let dir = tempfile::tempdir().unwrap();
    let db = dir.path().join("demo.db");
    seed_sqlite(&db, SEED);
    let (win, ws) = open_workspace(cx);
    let conn = connect_sqlite(cx, &ws, "demo", &db);
    cx.update_window(win, |_, window, cx| {
        ws.update(cx, |ws, cx| {
            ws.dispatch_open_tab(TabSpec::blank_query_editor(conn), window, cx)
        });
    })
    .unwrap();
    settle(cx);
    let panel = editors(&ws, cx).pop().expect("query tab");
    let s = panel.read_with(cx, |p, cx| p.result_snapshot(cx));
    assert!(
        !s.running && s.rows.is_empty(),
        "a blank tab must not run its starter SQL"
    );
}
