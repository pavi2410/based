//! Assistant access: "AI off" must keep everything on the machine.

use gpui_kit::TestAppContext;

use crate::common::{connect_sqlite, open_workspace, seed_sqlite, settle};

#[gpui_kit::test]
fn ai_off_connection_sends_nothing(cx: &mut TestAppContext) {
    let dir = tempfile::tempdir().expect("tempdir");
    let db = dir.path().join("a.db");
    seed_sqlite(&db, &["create table t (id integer primary key)"]);
    let (_win, ws) = open_workspace(cx);
    // New connections start with AI off.
    let conn = connect_sqlite(cx, &ws, "a", &db);

    ws.update(cx, |ws, cx| {
        ws.ask_assistant(&conn, "what is in t?".into(), cx)
    });
    settle(cx);

    let turns = ws.read_with(cx, |ws, cx| ws.chat().read(cx).turn_count());
    assert_eq!(
        turns, 0,
        "nothing may be queued for the provider while AI is off"
    );
}
