//! Workspace startup over an injected project / personal tree.

use std::fs;
use std::path::Path;

use based_desktop::connection::ConnectionOrigin;
use based_desktop::project::loader::SkippedConnection;
use based_desktop::workspace::Workspace;
use based_desktop::workspace::boot::WorkspaceEnv;
use based_desktop::workspace::notify::{remember_skipped_notice, skipped_connections_notice};
use gpui_kit::component::WindowExt as _;
use gpui_kit::{AnyWindowHandle, AppContext as _, Entity, TestAppContext};

use crate::common::{no_project_env, open_project, open_with, open_workspace, settle};

const SQLITE: &str =
    "schema_version = 1\nlabel = \"Demo\"\nengine = \"sqlite\"\nfile = \"demo.db\"\n";

fn write_connection(based_dir: &Path, name: &str) {
    let dir = based_dir.join("connections");
    fs::create_dir_all(&dir).unwrap();
    fs::write(dir.join(format!("{name}.toml")), SQLITE).unwrap();
}

fn origins(ws: &Entity<Workspace>, cx: &mut TestAppContext) -> Vec<ConnectionOrigin> {
    ws.read_with(cx, |ws, cx| {
        ws.registry()
            .read(cx)
            .connections()
            .iter()
            .map(|c| c.read(cx).origin)
            .collect()
    })
}

#[gpui_kit::test]
fn no_project_means_no_connections(cx: &mut TestAppContext) {
    let (_win, ws) = open_workspace(cx);
    assert!(origins(&ws, cx).is_empty());
}

#[gpui_kit::test]
fn loads_project_connections(cx: &mut TestAppContext) {
    let project = tempfile::tempdir().unwrap();
    write_connection(&project.path().join(".based"), "demo");
    let (_win, ws) = open_project(cx, project.path());
    settle(cx);
    assert_eq!(origins(&ws, cx), vec![ConnectionOrigin::Project]);
}

#[gpui_kit::test]
fn loads_personal_connections(cx: &mut TestAppContext) {
    let personal = tempfile::tempdir().unwrap();
    write_connection(personal.path(), "mine");
    let env = WorkspaceEnv {
        personal_root: personal.path().to_path_buf(),
        ..no_project_env()
    };
    let (_win, ws) = open_with(cx, env);
    settle(cx);
    assert_eq!(origins(&ws, cx), vec![ConnectionOrigin::Personal]);
}

#[gpui_kit::test]
fn skipped_connections_notice_shows_once_per_change(cx: &mut TestAppContext) {
    let dir = Path::new("/tmp/based-notice-test/.based");
    let missing = vec![SkippedConnection {
        id: "prod".into(),
        missing_var: Some("BASED_PROD_PASSWORD".into()),
        reason: "environment variable `BASED_PROD_PASSWORD` is not set".into(),
    }];
    cx.update(|cx| {
        assert!(skipped_connections_notice(cx, dir, &missing).is_some());
        assert!(
            skipped_connections_notice(cx, dir, &missing).is_some(),
            "not shown yet (no window took it): still pending"
        );
        remember_skipped_notice(cx, dir, &missing);
        assert!(
            skipped_connections_notice(cx, dir, &missing).is_none(),
            "a reload with the same problem stays quiet"
        );
        assert!(
            skipped_connections_notice(cx, dir, &[]).is_none(),
            "fixed: nothing to say"
        );
        assert!(
            skipped_connections_notice(cx, dir, &missing).is_some(),
            "broken again: say so again"
        );
    });
}

fn notice_count(win: AnyWindowHandle, cx: &mut TestAppContext) -> usize {
    cx.update_window(win, |_, window, cx| window.notifications(cx).len())
        .unwrap()
}

#[gpui_kit::test]
fn clean_startup_shows_no_notice(cx: &mut TestAppContext) {
    let project = tempfile::tempdir().unwrap();
    write_connection(&project.path().join(".based"), "demo");
    let (win, _ws) = open_project(cx, project.path());
    settle(cx);
    assert_eq!(notice_count(win, cx), 0);
}

#[gpui_kit::test]
fn startup_names_connections_missing_a_secret(cx: &mut TestAppContext) {
    let project = tempfile::tempdir().unwrap();
    let dir = project.path().join(".based").join("connections");
    fs::create_dir_all(&dir).unwrap();
    fs::write(
        dir.join("prod.toml"),
        "schema_version = 1\nlabel = \"Prod\"\nengine = \"postgres\"\nhost = \"db\"\n\
         database = \"app\"\nusername = \"app\"\n\
         password = { env = \"BASED_UI_TEST_MISSING_SECRET\" }\n",
    )
    .unwrap();
    let (win, ws) = open_project(cx, project.path());
    settle(cx);
    assert!(origins(&ws, cx).is_empty());
    assert_eq!(notice_count(win, cx), 1);
}
