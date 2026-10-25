//! Data viewer: paging, filtering and refresh against a real SQLite table.

use std::path::PathBuf;

use based_desktop::app::prefs;
use based_desktop::connection::ConnectionId;
use based_desktop::workspace::panels::data_view::PageSnapshot;
use based_desktop::workspace::panels::data_viewer::DataViewerPanel;
use based_desktop::workspace::tabs::DrillFilter;
use based_desktop::workspace::{TabSpec, Workspace};
use gpui_kit::test::TestWindowExt;
use gpui_kit::{AnyWindowHandle, AppContext, Entity, TestAppContext};

use crate::common::{connect_sqlite, open_workspace, seed_sqlite, settle, wait_until};

const SEED: &[&str] = &[
    "create table items (id integer primary key, grp text, label text)",
    "with recursive n(i) as (select 1 union all select i + 1 from n where i < 120)
     insert into items select i, case when i % 2 = 0 then 'even' else 'odd' end, 'item' || i from n",
];

struct Viewer {
    win: AnyWindowHandle,
    panel: Entity<DataViewerPanel>,
    db: PathBuf,
    _dir: tempfile::TempDir,
}

impl Viewer {
    fn snap(&self, cx: &mut TestAppContext) -> PageSnapshot {
        self.panel.read_with(cx, |p, cx| p.page_snapshot(cx))
    }

    /// Wait until a load finished and rows (or an empty total) are in.
    fn loaded(&self, cx: &mut TestAppContext) -> PageSnapshot {
        wait_until(cx, |cx| {
            let s = self.snap(cx);
            !s.loading && !s.columns.is_empty()
        });
        self.snap(cx)
    }

    fn click(&self, id: &'static str, cx: &mut TestAppContext) {
        cx.update_window(self.win, |_, window, cx| {
            window.render_frame(cx);
            window.click(id, cx);
        })
        .unwrap();
        settle(cx);
    }
}

fn open_viewer(cx: &mut TestAppContext, filter: Option<DrillFilter>) -> Viewer {
    let dir = tempfile::tempdir().unwrap();
    let db = dir.path().join("demo.db");
    seed_sqlite(&db, SEED);
    let (win, ws) = open_workspace(cx);
    cx.update(|cx| prefs::set_page_size(50, cx));
    let conn = connect_sqlite(cx, &ws, "demo", &db);
    open_tab(cx, win, &ws, conn, filter);
    let panel = ws.read_with(cx, |ws, cx| {
        ws.tab_manager()
            .read(cx)
            .tabs
            .iter()
            .find_map(|t| t.view.clone().downcast::<DataViewerPanel>().ok())
            .expect("data viewer tab")
    });
    Viewer {
        win,
        panel,
        db,
        _dir: dir,
    }
}

fn open_tab(
    cx: &mut TestAppContext,
    win: AnyWindowHandle,
    ws: &Entity<Workspace>,
    conn_id: ConnectionId,
    filter: Option<DrillFilter>,
) {
    let spec = TabSpec::DataViewer {
        conn_id,
        object: "items".into(),
        filter,
    };
    cx.update_window(win, |_, window, cx| {
        assert!(ws.update(cx, |ws, cx| ws.dispatch_open_tab(spec, window, cx)));
        window.render_frame(cx);
    })
    .unwrap();
    settle(cx);
}

#[gpui_kit::test]
fn first_page_loads(cx: &mut TestAppContext) {
    let v = open_viewer(cx, None);
    let s = v.loaded(cx);
    assert_eq!(s.columns, ["id", "grp", "label"]);
    assert_eq!((s.total, s.offset, s.rows.len()), (120, 0, 50));
    assert_eq!(s.rows[0], ["1", "odd", "item1"]);
}

#[gpui_kit::test]
fn paging_moves_the_window(cx: &mut TestAppContext) {
    let v = open_viewer(cx, None);
    v.loaded(cx);
    v.panel.update(cx, |p, cx| p.show_page(3, cx));
    wait_until(cx, |cx| v.snap(cx).offset == 100 && !v.snap(cx).loading);
    let s = v.snap(cx);
    assert_eq!(s.rows.len(), 20, "last page is partial");
    assert_eq!(s.rows[0][0], "101");
}

#[gpui_kit::test]
fn page_size_pref_applies_to_new_viewers(cx: &mut TestAppContext) {
    let (win, ws) = open_workspace(cx);
    cx.update(|cx| prefs::set_page_size(100, cx));
    let dir = tempfile::tempdir().unwrap();
    let db = dir.path().join("demo.db");
    seed_sqlite(&db, SEED);
    let conn = connect_sqlite(cx, &ws, "demo", &db);
    open_tab(cx, win, &ws, conn, None);
    let panel = ws.read_with(cx, |ws, cx| {
        ws.tab_manager()
            .read(cx)
            .tabs
            .iter()
            .find_map(|t| t.view.clone().downcast::<DataViewerPanel>().ok())
            .unwrap()
    });
    wait_until(cx, |cx| {
        panel.read_with(cx, |p, cx| p.page_snapshot(cx).rows.len() == 100)
    });
}

#[gpui_kit::test]
fn opens_with_a_drill_filter(cx: &mut TestAppContext) {
    let v = open_viewer(
        cx,
        Some(DrillFilter {
            column: "grp".into(),
            value: "even".into(),
        }),
    );
    wait_until(cx, |cx| v.snap(cx).total == 60);
    let s = v.snap(cx);
    assert!(s.rows.iter().all(|r| r[1] == "even"), "{:?}", s.rows);
}

#[gpui_kit::test]
fn clear_filter_restores_all_rows(cx: &mut TestAppContext) {
    let v = open_viewer(
        cx,
        Some(DrillFilter {
            column: "grp".into(),
            value: "even".into(),
        }),
    );
    wait_until(cx, |cx| v.snap(cx).total == 60);
    v.click("sqlite-dv-filter-clear", cx);
    wait_until(cx, |cx| v.snap(cx).total == 120);
}

#[gpui_kit::test]
fn refresh_picks_up_new_rows(cx: &mut TestAppContext) {
    let v = open_viewer(cx, None);
    v.loaded(cx);
    seed_sqlite(&v.db, &["insert into items values (121, 'odd', 'item121')"]);
    v.click("sqlite-dv-refresh", cx);
    wait_until(cx, |cx| v.snap(cx).total == 121);
}
