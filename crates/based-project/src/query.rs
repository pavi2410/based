use std::fs;
use std::path::Path;

use anyhow::{Context, Result, bail};
use serde::Deserialize;
use toml_edit::{DocumentMut, value};

use crate::target::QueryTarget;
use crate::walk::{rel_id, walk_toml_files};

pub const QUERY_SCHEMA_VERSION: u64 = 1;

#[derive(Debug, Clone)]
pub struct ProjectQuery {
    /// Path relative to `.based/queries/` without the `.toml` suffix
    /// (e.g. `local/northwind/recent-orders`).
    pub path: String,
    pub name: String,
    pub description: Option<String>,
    pub tags: Vec<String>,
    pub target: QueryTarget,
    pub body: QueryBody,
}

#[derive(Debug, Clone)]
pub enum QueryBody {
    Sql {
        query: String,
    },
    Aggregate {
        pipeline: String,
        collection: Option<String>,
    },
}

#[derive(Debug, Deserialize)]
struct QueryFileRaw {
    schema_version: u64,
    name: String,
    #[serde(default)]
    description: Option<String>,
    #[serde(default)]
    tags: Vec<String>,
    target: QueryTarget,
    sql: Option<SqlSection>,
    aggregate: Option<AggregateSection>,
}

#[derive(Debug, Deserialize)]
struct SqlSection {
    query: String,
}

#[derive(Debug, Deserialize)]
struct AggregateSection {
    #[serde(default)]
    collection: Option<String>,
    pipeline: String,
}

/// Where a saved query runs.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SaveTarget {
    /// One project connection, by its id.
    Connection(String),
    /// Any connection of this engine (`postgres`, `sqlite`, `mongodb`, `redis`).
    Engine(String),
}

#[derive(serde::Serialize)]
struct QueryFileOut<'a> {
    schema_version: u64,
    name: &'a str,
    target: TargetOut<'a>,
    sql: SqlOut<'a>,
}

#[derive(serde::Serialize)]
struct TargetOut<'a> {
    #[serde(skip_serializing_if = "Option::is_none")]
    connection: Option<&'a str>,
    #[serde(skip_serializing_if = "Option::is_none")]
    engine: Option<&'a str>,
}

#[derive(serde::Serialize)]
struct SqlOut<'a> {
    query: &'a str,
}

/// File-name stem for a query name: lowercase words joined by `-`.
pub fn slugify(name: &str) -> String {
    let mut out = String::new();
    for c in name.chars() {
        if c.is_alphanumeric() {
            out.extend(c.to_lowercase());
        } else if !out.is_empty() && !out.ends_with('-') {
            out.push('-');
        }
    }
    let out = out.trim_end_matches('-').to_string();
    if out.is_empty() { "query".into() } else { out }
}

/// Write a SQL query into `.based/queries/` and return its project-relative path
/// (no `.toml`). With `overwrite` an existing `[sql]` file keeps everything but its
/// name and query text (description, tags, target, comments); otherwise a numeric
/// suffix keeps an existing file safe.
pub fn save_sql_query(
    project_root: &Path,
    name: &str,
    sql: &str,
    target: &SaveTarget,
    overwrite: Option<&str>,
) -> Result<String> {
    let dir = project_root.join(".based").join("queries");
    fs::create_dir_all(&dir).with_context(|| format!("create {}", dir.display()))?;
    let rel = match overwrite {
        Some(path) => path.to_string(),
        None => {
            let base = slugify(name);
            let mut candidate = base.clone();
            let mut n = 2;
            while dir.join(format!("{candidate}.toml")).exists() {
                candidate = format!("{base}-{n}");
                n += 1;
            }
            candidate
        }
    };
    let file = dir.join(format!("{rel}.toml"));
    if overwrite.is_some()
        && let Some(text) = update_sql_in_place(&file, name, sql)
    {
        fs::write(&file, text).with_context(|| format!("write {}", file.display()))?;
        return Ok(rel);
    }
    let (connection, engine) = match target {
        SaveTarget::Connection(id) => (Some(id.as_str()), None),
        SaveTarget::Engine(e) => (None, Some(e.as_str())),
    };
    let out = QueryFileOut {
        schema_version: QUERY_SCHEMA_VERSION,
        name,
        target: TargetOut { connection, engine },
        sql: SqlOut { query: sql },
    };
    let text = toml::to_string(&out).context("encode query")?;
    if let Some(parent) = file.parent() {
        fs::create_dir_all(parent)?;
    }
    fs::write(&file, text).with_context(|| format!("write {}", file.display()))?;
    Ok(rel)
}

/// The existing query file with only `name` and `[sql] query` replaced, or `None`
/// when there is no `[sql]` file to update (missing, unreadable, `[aggregate]`).
fn update_sql_in_place(file: &Path, name: &str, sql: &str) -> Option<String> {
    let raw = fs::read_to_string(file).ok()?;
    let mut doc: DocumentMut = raw.parse().ok()?;
    let table = doc.get_mut("sql")?.as_table_mut()?;
    table["query"] = value(sql);
    doc["name"] = value(name);
    Some(doc.to_string())
}

/// A query file that could not be loaded, and why.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SkippedQuery {
    /// Path under `.based/queries/`, without the `.toml` suffix.
    pub path: String,
    pub reason: String,
}

/// Saved queries, plus the files that did not parse. One bad file never hides
/// the others.
#[derive(Debug, Clone, Default)]
pub struct LoadedQueries {
    pub queries: Vec<ProjectQuery>,
    pub skipped: Vec<SkippedQuery>,
}

pub fn load_queries(project_root: &Path) -> Result<LoadedQueries> {
    let dir = project_root.join(".based").join("queries");
    let mut out = LoadedQueries::default();
    if !dir.is_dir() {
        return Ok(out);
    }
    for path in walk_toml_files(&dir)? {
        match parse_query_file(&dir, &path) {
            Ok(q) => out.queries.push(q),
            Err(e) => out.skipped.push(SkippedQuery {
                path: path
                    .strip_prefix(&dir)
                    .map(rel_id)
                    .unwrap_or_else(|_| path.display().to_string()),
                reason: format!("{e:#}"),
            }),
        }
    }
    Ok(out)
}

fn parse_query_file(queries_dir: &Path, path: &Path) -> Result<ProjectQuery> {
    let rel = path
        .strip_prefix(queries_dir)
        .with_context(|| format!("query path not under {}", queries_dir.display()))?;
    let rel_path = rel_id(rel);
    let raw = fs::read_to_string(path).with_context(|| format!("read {}", path.display()))?;
    let file: QueryFileRaw =
        toml::from_str(&raw).with_context(|| format!("parse {}", path.display()))?;
    if file.schema_version != QUERY_SCHEMA_VERSION {
        bail!(
            "unsupported schema_version {} in {} (expected {QUERY_SCHEMA_VERSION})",
            file.schema_version,
            path.display()
        );
    }
    let has_sql = file.sql.is_some();
    let has_agg = file.aggregate.is_some();
    if has_sql == has_agg {
        bail!(
            "query {} must have exactly one of [sql] or [aggregate]",
            path.display()
        );
    }
    file.target.validate()?;
    let body = if let Some(sql) = file.sql {
        QueryBody::Sql { query: sql.query }
    } else if let Some(agg) = file.aggregate {
        QueryBody::Aggregate {
            pipeline: agg.pipeline,
            collection: agg.collection,
        }
    } else {
        unreachable!()
    };
    Ok(ProjectQuery {
        path: rel_path,
        name: file.name,
        description: file.description,
        tags: file.tags,
        target: file.target,
        body,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    fn write_sql_query(path: &Path, name: &str) {
        fs::write(
            path,
            format!(
                r#"
schema_version = 1
name = "{name}"

[target]
engine = "sqlite"

[sql]
query = "SELECT 1"
"#
            ),
        )
        .unwrap();
    }

    #[test]
    fn query_identity_strips_toml_suffix() {
        let dir = tempfile::tempdir().unwrap();
        let queries = dir.path().join("queries");
        fs::create_dir_all(queries.join("local/northwind")).unwrap();
        let path = queries.join("local/northwind/recent-orders.toml");
        write_sql_query(&path, "Recent Orders");
        let query = parse_query_file(&queries, &path).unwrap();
        assert_eq!(query.path, "local/northwind/recent-orders");
    }

    #[test]
    fn query_identity_strips_legacy_query_toml_suffix() {
        let dir = tempfile::tempdir().unwrap();
        let queries = dir.path().join("queries");
        fs::create_dir_all(&queries).unwrap();
        let path = queries.join("list-tables.query.toml");
        write_sql_query(&path, "List Tables");
        let query = parse_query_file(&queries, &path).unwrap();
        assert_eq!(query.path, "list-tables");
    }

    #[test]
    fn slugify_makes_file_stems() {
        assert_eq!(slugify("Weekly active users!"), "weekly-active-users");
        assert_eq!(slugify("  --  "), "query");
    }

    #[test]
    fn saved_query_round_trips_and_does_not_clobber() {
        let dir = tempfile::tempdir().unwrap();
        let target = SaveTarget::Connection("local/northwind".into());
        let sql = "select 1;\nselect 'it''s'";
        let a = save_sql_query(dir.path(), "My Query", sql, &target, None).unwrap();
        let b = save_sql_query(dir.path(), "My Query", "select 2", &target, None).unwrap();
        assert_eq!((a.as_str(), b.as_str()), ("my-query", "my-query-2"));
        let loaded = load_queries(dir.path()).unwrap().queries;
        assert_eq!(loaded.len(), 2);
        let first = loaded.iter().find(|q| q.path == "my-query").unwrap();
        assert_eq!(first.name, "My Query");
        assert!(matches!(&first.body, QueryBody::Sql { query } if query == sql));
        // Saving again at the same path replaces it.
        save_sql_query(
            dir.path(),
            "My Query",
            "select 3",
            &target,
            Some("my-query"),
        )
        .unwrap();
        let loaded = load_queries(dir.path()).unwrap().queries;
        let again = loaded.iter().find(|q| q.path == "my-query").unwrap();
        assert!(matches!(&again.body, QueryBody::Sql { query } if query == "select 3"));
    }

    #[test]
    fn overwriting_keeps_description_tags_and_target() {
        let dir = tempfile::tempdir().unwrap();
        let queries = dir.path().join(".based").join("queries");
        fs::create_dir_all(&queries).unwrap();
        fs::write(
            queries.join("top.toml"),
            r#"# Shared report, keep the target generic.
schema_version = 1
name = "Top users"
description = "Most active users this week"
tags = ["reports"]

[target]
engine = "postgres"

[sql]
query = "select 1"
"#,
        )
        .unwrap();
        save_sql_query(
            dir.path(),
            "Top users",
            "select 2",
            &SaveTarget::Connection("my-local-pg".into()),
            Some("top"),
        )
        .unwrap();
        let raw = fs::read_to_string(queries.join("top.toml")).unwrap();
        assert!(raw.starts_with("# Shared report"), "{raw}");
        let q = load_queries(dir.path()).unwrap().queries.remove(0);
        assert!(matches!(&q.body, QueryBody::Sql { query } if query == "select 2"));
        assert_eq!(
            q.description.as_deref(),
            Some("Most active users this week")
        );
        assert_eq!(q.tags, ["reports"]);
        assert_eq!(q.target.engine.as_deref(), Some("postgres"));
        assert!(q.target.connection.is_none());
    }

    #[test]
    fn engine_target_saves_without_a_connection() {
        let dir = tempfile::tempdir().unwrap();
        save_sql_query(
            dir.path(),
            "q",
            "select 1",
            &SaveTarget::Engine("sqlite".into()),
            None,
        )
        .unwrap();
        let q = load_queries(dir.path()).unwrap().queries.remove(0);
        assert_eq!(q.target.engine.as_deref(), Some("sqlite"));
        assert!(q.target.connection.is_none());
    }

    #[test]
    fn one_bad_query_file_does_not_hide_the_others() {
        let dir = tempfile::tempdir().unwrap();
        let target = SaveTarget::Connection("local/northwind".into());
        save_sql_query(dir.path(), "Good", "select 1", &target, None).unwrap();
        let queries = dir.path().join(".based").join("queries");
        fs::write(queries.join("broken.toml"), "schema_version = 1\nname = ").unwrap();
        let loaded = load_queries(dir.path()).unwrap();
        assert_eq!(loaded.queries.len(), 1);
        assert_eq!(loaded.skipped.len(), 1);
        assert_eq!(loaded.skipped[0].path, "broken");
    }
}
