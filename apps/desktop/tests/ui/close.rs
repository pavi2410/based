//! Bulk close commands, pinned tabs and unsaved (dirty) query tabs.

use based_desktop::connection::ConnectionId;
use based_desktop::workspace::panels::query_editor::QueryEditorPanel;
use based_desktop::workspace::{TabSpec, Workspace};
use gpui_kit::test::TestWindowExt;
use gpui_kit::{AnyWindowHandle, AppContext, Entity, EntityId, TestAppContext};

use crate::common::{connect_sqlite, open_workspace, seed_sqlite, settle};

fn info(name: &str) -> TabSpec {
    TabSpec::ObjectInfo {
        conn_id: ConnectionId("t".into()),
        object_name: name.into(),
        kind_label: "Trigger".into(),
    }
}

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
                .view
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
    s.run(cx, |ws, w, cx| ws.close_other_tabs(b, w, cx));
    assert_eq!(s.specs(cx), [info("b")]);
}

#[gpui_kit::test]
fn close_all_tabs_leaves_home(cx: &mut TestAppContext) {
    let s = strip(cx);
    s.run(cx, |ws, w, cx| ws.close_all_tabs(w, cx));
    assert_eq!(s.specs(cx), [TabSpec::Home]);
}

#[gpui_kit::test]
fn close_to_the_left_and_right(cx: &mut TestAppContext) {
    let s = strip(cx);
    let b = s.id_of(&info("b"), cx);
    s.run(cx, |ws, w, cx| ws.close_tabs_to_right(b, w, cx));
    assert_eq!(s.specs(cx), [TabSpec::Home, info("a"), info("b")]);
    s.run(cx, |ws, w, cx| ws.close_tabs_to_left(b, w, cx));
    assert_eq!(s.specs(cx), [info("b")]);
}

#[gpui_kit::test]
fn bulk_close_skips_pinned_tabs(cx: &mut TestAppContext) {
    let s = strip(cx);
    let a = s.id_of(&info("a"), cx);
    s.run(cx, |ws, _, cx| ws.toggle_pin_tab(a, cx));
    s.run(cx, |ws, w, cx| ws.close_all_tabs(w, cx));
    assert_eq!(s.specs(cx), [info("a")]);
    s.run(cx, |ws, w, cx| ws.close_center_panel(a, w, cx));
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
                .find_map(|t| t.view.clone().downcast::<QueryEditorPanel>().ok())
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
    q.s.run(cx, |ws, w, cx| ws.close_clean_tabs(w, cx));
    let specs = q.s.specs(cx);
    assert_eq!(specs.len(), 1, "{specs:?}");
    assert!(matches!(specs[0], TabSpec::QueryEditor { .. }));
    assert_eq!(q.s.dirty(cx), [true]);
}
