use std::path::Path;

use anyhow::{Context, Result, bail};
use serde::Deserialize;

use crate::target::QueryTarget;
use crate::walk::walk_files;

pub const QUERY_SCHEMA_VERSION: u64 = 1;

#[derive(Debug, Clone)]
pub struct ProjectQuery {
    /// Path relative to `.based/queries/` (e.g. `local/northwind/recent-orders.query.toml`).
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

pub fn load_queries(project_root: &Path) -> Result<Vec<ProjectQuery>> {
    let dir = project_root.join(".based").join("queries");
    if !dir.is_dir() {
        return Ok(vec![]);
    }
    let files = walk_files(&dir, ".query.toml")?;
    let mut queries = Vec::with_capacity(files.len());
    for path in files {
        queries.push(parse_query_file(&dir, &path)?);
    }
    Ok(queries)
}

fn parse_query_file(queries_dir: &Path, path: &Path) -> Result<ProjectQuery> {
    let rel = path
        .strip_prefix(queries_dir)
        .with_context(|| format!("query path not under {}", queries_dir.display()))?;
    let rel_path = rel.to_string_lossy().replace('\\', "/");
    let raw = std::fs::read_to_string(path).with_context(|| format!("read {}", path.display()))?;
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
