//! Workspace startup over an injected project / personal tree.

use std::fs;
use std::path::Path;

use based_desktop::connection::ConnectionOrigin;
use based_desktop::workspace::Workspace;
use based_desktop::workspace::boot::WorkspaceEnv;
use gpui_kit::{Entity, TestAppContext};

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
