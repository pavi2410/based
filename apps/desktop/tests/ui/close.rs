//! Bulk close commands, pinned tabs and unsaved (dirty) query tabs.

use based_desktop::workspace::panels::query_editor::QueryEditorPanel;
use based_desktop::workspace::tabs::{CloseOutcome, CloseScope};
use based_desktop::workspace::{TabSpec, Workspace};
use gpui_kit::test::TestWindowExt;
use gpui_kit::{AnyWindowHandle, AppContext, Entity, EntityId, TestAppContext};

use crate::common::{connect_sqlite, info, open_workspace, seed_sqlite, settle};

struct Strip {
    win: AnyWindowHandle,
    ws: Entity<Workspace>,
}

impl Strip {
    fn open(&self, spec: TabSpec, cx: &mut TestAppContext) {
        cx.update_window(self.win, |_, window, cx| {
            assert!(
                self.ws
                    .update(cx, |ws, cx| ws.dispatch_open_tab(spec, window, cx))
            );
            window.render_frame(cx);
        })
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

    fn id_of(&self, spec: &TabSpec, cx: &mut TestAppContext) -> EntityId {
        self.ws.read_with(cx, |ws, cx| {
            let tm = ws.tab_manager().read(cx);
            tm.tabs
                .iter()
                .find(|t| t.spec == *spec)
                .expect("tab")
                .view()
                .entity_id()
        })
    }

    fn run(
        &self,
        cx: &mut TestAppContext,
        f: impl FnOnce(&mut Workspace, &mut gpui_kit::Window, &mut gpui_kit::Context<Workspace>),
    ) {
        cx.update_window(self.win, |_, window, cx| {
            self.ws.update(cx, |ws, cx| f(ws, window, cx));
            window.render_frame(cx);
        })
        .unwrap();
        settle(cx);
    }

    fn close(&self, scope: CloseScope, cx: &mut TestAppContext) -> CloseOutcome {
        let out = cx
            .update_window(self.win, |_, window, cx| {
                self.ws
                    .update(cx, |ws, cx| ws.close_tabs(scope, window, cx))
            })
            .unwrap();
        settle(cx);
        out
    }

    fn prompt_dirty(&self, cx: &mut TestAppContext) -> Option<usize> {
        self.ws
            .read_with(cx, |ws, _| ws.close_prompt().map(|p| p.dirty()))
    }

    fn dirty(&self, cx: &mut TestAppContext) -> Vec<bool> {
        self.ws.read_with(cx, |ws, cx| {
            ws.tab_manager()
                .read(cx)
                .tabs
                .iter()
                .map(|t| t.dirty)
                .collect()
        })
    }
}

/// Home, a, b, c.
fn strip(cx: &mut TestAppContext) -> Strip {
    let (win, ws) = open_workspace(cx);
    let s = Strip { win, ws };
    for n in ["a", "b", "c"] {
        s.open(info(n), cx);
    }
    s
}

#[gpui_kit::test]
fn close_other_tabs_keeps_only_the_anchor(cx: &mut TestAppContext) {
    let s = strip(cx);
    let b = s.id_of(&info("b"), cx);
    s.close(CloseScope::Others(b), cx);
    assert_eq!(s.specs(cx), [info("b")]);
}

#[gpui_kit::test]
fn close_all_tabs_leaves_home(cx: &mut TestAppContext) {
    let s = strip(cx);
    s.close(CloseScope::All, cx);
    assert_eq!(s.specs(cx), [TabSpec::Home]);
}

#[gpui_kit::test]
fn close_to_the_left_and_right(cx: &mut TestAppContext) {
    let s = strip(cx);
    let b = s.id_of(&info("b"), cx);
    s.close(CloseScope::Right(b), cx);
    assert_eq!(s.specs(cx), [TabSpec::Home, info("a"), info("b")]);
    s.close(CloseScope::Left(b), cx);
    assert_eq!(s.specs(cx), [info("b")]);
}

#[gpui_kit::test]
fn bulk_close_skips_pinned_tabs(cx: &mut TestAppContext) {
    let s = strip(cx);
    let a = s.id_of(&info("a"), cx);
    s.run(cx, |ws, _, cx| ws.toggle_pin_tab(a, cx));
    s.close(CloseScope::All, cx);
    assert_eq!(s.specs(cx), [info("a")]);
    s.close(CloseScope::Tab(a), cx);
    assert_eq!(
        s.specs(cx),
        [info("a")],
        "a pinned tab cannot be closed directly"
    );
}

struct Sql {
    s: Strip,
    _dir: tempfile::TempDir,
}

impl Sql {
    /// Type into the focused query editor.
    fn type_sql(&self, text: &str, cx: &mut TestAppContext) {
        let panel = self.s.ws.read_with(cx, |ws, cx| {
            ws.tab_manager()
                .read(cx)
                .tabs
                .iter()
                .find_map(|t| t.downcast::<QueryEditorPanel>())
                .expect("query editor")
        });
        cx.update_window(self.s.win, |_, window, cx| {
            panel.update(cx, |p, cx| p.focus_editor(window, cx));
            window.input(text, cx);
            window.render_frame(cx);
        })
        .unwrap();
        settle(cx);
    }
}

/// A connected SQLite database with a blank query tab open.
fn with_query(cx: &mut TestAppContext) -> Sql {
    let dir = tempfile::tempdir().unwrap();
    let db = dir.path().join("demo.db");
    seed_sqlite(&db, &["create table t (id integer)"]);
    let (win, ws) = open_workspace(cx);
    let conn = connect_sqlite(cx, &ws, "demo", &db);
    let s = Strip { win, ws };
    s.open(TabSpec::blank_query_editor(conn.clone()), cx);
    Sql { s, _dir: dir }
}

#[gpui_kit::test]
fn typing_marks_the_query_tab_dirty(cx: &mut TestAppContext) {
    let q = with_query(cx);
    assert!(
        !q.s.ws.read_with(cx, |ws, cx| ws.has_dirty_tabs(cx)),
        "blank tab is clean"
    );
    q.type_sql("select 1", cx);
    assert!(q.s.ws.read_with(cx, |ws, cx| ws.has_dirty_tabs(cx)));
}

#[gpui_kit::test]
fn close_clean_tabs_keeps_dirty_ones(cx: &mut TestAppContext) {
    let q = with_query(cx);
    q.type_sql("select 1", cx);
    q.s.open(info("a"), cx);
    q.s.close(CloseScope::Clean, cx);
    let specs = q.s.specs(cx);
    assert_eq!(specs.len(), 1, "{specs:?}");
    assert!(matches!(specs[0], TabSpec::QueryEditor { .. }));
    assert_eq!(q.s.dirty(cx), [true]);
}

#[gpui_kit::test]
fn closing_a_dirty_tab_asks_first(cx: &mut TestAppContext) {
    let q = with_query(cx);
    q.type_sql("select 1", cx);
    assert_eq!(
        q.s.close(CloseScope::Active, cx),
        CloseOutcome::NeedsConfirm
    );
    assert_eq!(q.s.prompt_dirty(cx), Some(1));
    assert_eq!(q.s.specs(cx).len(), 2, "nothing closed yet");
}

#[gpui_kit::test]
fn cancelling_keeps_the_dirty_tab(cx: &mut TestAppContext) {
    let q = with_query(cx);
    q.type_sql("select 1", cx);
    q.s.close(CloseScope::Active, cx);
    q.s.run(cx, |ws, _, cx| ws.cancel_close_prompt(cx));
    assert_eq!(q.s.prompt_dirty(cx), None);
    assert_eq!(q.s.dirty(cx), [false, true]);
}

#[gpui_kit::test]
fn confirming_closes_the_dirty_tab(cx: &mut TestAppContext) {
    let q = with_query(cx);
    q.type_sql("select 1", cx);
    q.s.close(CloseScope::Active, cx);
    q.s.run(cx, |ws, w, cx| ws.confirm_close_prompt(w, cx));
    assert_eq!(q.s.specs(cx), [TabSpec::Home]);
    assert_eq!(q.s.prompt_dirty(cx), None);
}

#[gpui_kit::test]
fn bulk_close_holds_everything_until_confirmed(cx: &mut TestAppContext) {
    let q = with_query(cx);
    q.type_sql("select 1", cx);
    q.s.open(info("a"), cx);
    assert_eq!(q.s.close(CloseScope::All, cx), CloseOutcome::NeedsConfirm);
    assert_eq!(q.s.specs(cx).len(), 3, "clean tabs wait for the answer too");
    q.s.run(cx, |ws, w, cx| ws.confirm_close_prompt(w, cx));
    assert_eq!(q.s.specs(cx), [TabSpec::Home]);
}

#[gpui_kit::test]
fn closing_clean_tabs_never_asks(cx: &mut TestAppContext) {
    let q = with_query(cx);
    q.type_sql("select 1", cx);
    q.s.open(info("a"), cx);
    assert_eq!(q.s.close(CloseScope::Clean, cx), CloseOutcome::Closed);
    assert_eq!(q.s.prompt_dirty(cx), None);
}

#[gpui_kit::test]
fn forced_close_skips_the_prompt(cx: &mut TestAppContext) {
    let q = with_query(cx);
    q.type_sql("select 1", cx);
    q.s.run(cx, |ws, w, cx| ws.force_close_tabs(CloseScope::All, w, cx));
    assert_eq!(q.s.specs(cx), [TabSpec::Home]);
    assert_eq!(q.s.prompt_dirty(cx), None);
}

#[gpui_kit::test]
fn the_prompt_renders_a_dialog(cx: &mut TestAppContext) {
    let q = with_query(cx);
    q.type_sql("select 1", cx);
    q.s.close(CloseScope::Active, cx);
    q.s.run(cx, |_, _, _| {}); // render opens the dialog
    cx.update_window(q.s.win, |_, window, cx| {
        window.render_frame(cx);
        window
            .try_find("close-tabs-confirm")
            .expect("confirm button");
    })
    .unwrap();
}
