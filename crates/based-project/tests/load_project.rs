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
        engine: Some("postgres".into()),
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
        engine: Some("postgres".into()),
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
    std::fs::create_dir_all(based.join("queries")).unwrap();
    std::fs::write(
        based.join("project.toml"),
        "schema_version = 1\nname = \"t\"\n",
    )
    .unwrap();
    std::fs::write(based.join("queries/bad.query.toml"), raw).unwrap();
    let err = load_project(dir.path()).unwrap_err();
    assert!(err.to_string().contains("exclusive"));
}
