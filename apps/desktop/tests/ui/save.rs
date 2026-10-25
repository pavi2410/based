//! Saving a connection writes to the chosen tree and registers it.

use std::path::{Path, PathBuf};

use based_desktop::connection::{ConnectionConfig, ConnectionOrigin};
use based_desktop::workspace::Workspace;
use based_desktop::workspace::boot::WorkspaceEnv;
use based_desktop::workspace::connection_destination::ConnectionDestination;
use based_sqlite::config::SqliteConfig;
use gpui_kit::{Entity, TestAppContext};

use crate::common::{no_project_env, open_project, open_with, open_workspace, settle};

fn sqlite(label: &str) -> ConnectionConfig {
    ConnectionConfig::SQLite(SqliteConfig {
        label: label.into(),
        path: PathBuf::from("demo.db"),
        read_only: false,
        pragma: None,
    })
}

fn save(
    ws: &Entity<Workspace>,
    label: &str,
    dest: ConnectionDestination,
    cx: &mut TestAppContext,
) -> Result<(), String> {
    let config = sqlite(label);
    let result = ws.update(cx, |ws, cx| {
        ws.save_wizard_connection(config, dest, None, vec![], None, cx)
    });
    settle(cx);
    result.map(|_| ())
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

fn saved_file(based_dir: &Path, slug: &str) -> PathBuf {
    based_dir.join("connections").join(format!("{slug}.toml"))
}

#[gpui_kit::test]
fn saves_to_the_project(cx: &mut TestAppContext) {
    let project = tempfile::tempdir().unwrap();
    let (_win, ws) = open_project(cx, project.path());
    save(&ws, "Demo", ConnectionDestination::Project, cx).unwrap();
    assert!(saved_file(&project.path().join(".based"), "demo").is_file());
    assert_eq!(origins(&ws, cx), vec![ConnectionOrigin::Project]);
}

#[gpui_kit::test]
fn saves_to_the_personal_tree(cx: &mut TestAppContext) {
    let personal = tempfile::tempdir().unwrap();
    let env = WorkspaceEnv {
        personal_root: personal.path().to_path_buf(),
        ..no_project_env()
    };
    let (_win, ws) = open_with(cx, env);
    save(&ws, "Mine", ConnectionDestination::Personal, cx).unwrap();
    assert!(saved_file(personal.path(), "mine").is_file());
    assert_eq!(origins(&ws, cx), vec![ConnectionOrigin::Personal]);
}

#[gpui_kit::test]
fn project_destination_needs_an_open_project(cx: &mut TestAppContext) {
    let (_win, ws) = open_workspace(cx);
    let err = save(&ws, "Demo", ConnectionDestination::Project, cx).unwrap_err();
    assert!(err.contains("no project"), "{err}");
    assert!(origins(&ws, cx).is_empty());
}

#[gpui_kit::test]
fn duplicate_label_is_rejected(cx: &mut TestAppContext) {
    let project = tempfile::tempdir().unwrap();
    let (_win, ws) = open_project(cx, project.path());
    save(&ws, "Demo", ConnectionDestination::Project, cx).unwrap();
    let err = save(&ws, "demo", ConnectionDestination::Project, cx).unwrap_err();
    assert!(err.contains("already exists"), "{err}");
    assert_eq!(origins(&ws, cx).len(), 1);
}
