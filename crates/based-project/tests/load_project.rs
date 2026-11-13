use std::fs;
use std::path::Path;

use based_core::EngineKind;
use based_project::{
    ConnectionRef, QueryTarget, ResolveError, TargetConnection, load_project, resolve_target,
};

#[test]
fn loads_repo_based_project() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    let snap = load_project(&root).expect("load project");
    assert_eq!(snap.manifest.name, "based");
    assert!(!snap.connections.is_empty());
    assert!(!snap.queries.is_empty());
    assert!(snap.connections.iter().any(|c| c.id == "local/northwind"));
    assert!(
        snap.queries
            .iter()
            .any(|q| q.path == "local/northwind/recent-orders")
    );
}

#[test]
fn resolve_exclusive_target() {
    let target = QueryTarget {
        connection: Some(TargetConnection::Exclusive("local/northwind".into())),
        engine: None,
        tags: vec![],
        exclude_tags: vec![],
    };
    let conns = vec![ConnectionRef {
        id: "local/northwind".into(),
        engine: EngineKind::SQLite,
        tags: vec!["local".into()],
    }];
    assert_eq!(
        resolve_target(&target, &conns, None).unwrap(),
        "local/northwind"
    );
}

#[test]
fn resolve_filter_by_engine() {
    let target = QueryTarget {
        connection: None,
        engine: Some(EngineKind::Postgres),
        tags: vec![],
        exclude_tags: vec![],
    };
    let conns = vec![
        ConnectionRef {
            id: "local/northwind".into(),
            engine: EngineKind::SQLite,
            tags: vec![],
        },
        ConnectionRef {
            id: "public/ebi".into(),
            engine: EngineKind::Postgres,
            tags: vec!["public".into()],
        },
    ];
    assert_eq!(resolve_target(&target, &conns, None).unwrap(), "public/ebi");
}

#[test]
fn resolve_ambiguous() {
    let target = QueryTarget {
        connection: None,
        engine: Some(EngineKind::Postgres),
        tags: vec![],
        exclude_tags: vec![],
    };
    let conns = vec![
        ConnectionRef {
            id: "public/ebi".into(),
            engine: EngineKind::Postgres,
            tags: vec![],
        },
        ConnectionRef {
            id: "public/mindsdb".into(),
            engine: EngineKind::Postgres,
            tags: vec![],
        },
    ];
    match resolve_target(&target, &conns, None) {
        Err(ResolveError::Ambiguous(ids)) => {
            assert_eq!(ids.len(), 2);
        }
        other => panic!("expected ambiguous, got {other:?}"),
    }
}

#[test]
fn exclusive_target_rejects_extra_filters() {
    let raw = r#"
schema_version = 1
name = "Bad"
tags = []

[target]
connection = "local/northwind"
engine = "sqlite"

[sql]
query = "SELECT 1"
"#;
    let dir = tempfile::tempdir().unwrap();
    let based = dir.path().join(".based");
    fs::create_dir_all(based.join("queries")).unwrap();
    fs::write(
        based.join("project.toml"),
        "schema_version = 1\nname = \"t\"\n",
    )
    .unwrap();
    fs::write(based.join("queries/bad.toml"), raw).unwrap();
    // The bad file is reported, not fatal to the project.
    let snap = load_project(dir.path()).unwrap();
    assert!(snap.queries.is_empty());
    assert_eq!(snap.skipped_queries.len(), 1);
    assert_eq!(snap.skipped_queries[0].path, "bad");
    assert!(snap.skipped_queries[0].reason.contains("exclusive"));
}

#[test]
fn skips_underscore_prefixed_toml() {
    let dir = tempfile::tempdir().unwrap();
    let based = dir.path().join(".based");
    fs::create_dir_all(based.join("connections")).unwrap();
    fs::create_dir_all(based.join("queries")).unwrap();
    fs::write(
        based.join("project.toml"),
        "schema_version = 1\nname = \"t\"\n",
    )
    .unwrap();
    fs::write(
        based.join("connections/_template.toml"),
        r#"
schema_version = 1
label = "Template"
engine = "sqlite"
file = "missing.db"
"#,
    )
    .unwrap();
    fs::write(
        based.join("queries/_draft.toml"),
        r#"
schema_version = 1
name = "Draft"

[target]
engine = "sqlite"

[sql]
query = "SELECT 1"
"#,
    )
    .unwrap();
    let snap = load_project(dir.path()).unwrap();
    assert!(snap.connections.is_empty());
    assert!(snap.queries.is_empty());
}

#[test]
fn unknown_engine_skips_just_that_query() {
    let dir = tempfile::tempdir().unwrap();
    let based = dir.path().join(".based");
    fs::create_dir_all(based.join("queries")).unwrap();
    fs::write(
        based.join("project.toml"),
        "schema_version = 1\nname = \"t\"\n",
    )
    .unwrap();
    let query = |engine: &str| {
        format!(
            "schema_version = 1\nname = \"Q\"\n\n[target]\nengine = \"{engine}\"\n\n[sql]\nquery = \"SELECT 1\"\n"
        )
    };
    fs::write(based.join("queries/typo.toml"), query("mongdb")).unwrap();
    fs::write(based.join("queries/ok.toml"), query("postgresql")).unwrap();
    let snap = load_project(dir.path()).unwrap();
    assert_eq!(snap.queries.len(), 1);
    assert_eq!(snap.queries[0].target.engine, Some(EngineKind::Postgres));
    assert_eq!(snap.skipped_queries.len(), 1);
    assert_eq!(snap.skipped_queries[0].path, "typo");
    assert!(snap.skipped_queries[0].reason.contains("mongdb"));
}
