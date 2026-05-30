use std::path::Path;

use anyhow::Result;

use crate::connection::load_connections;
use crate::environment::load_active_environment;
use crate::favorites::load_favorites;
use crate::project::{ProjectManifest, load_manifest};
use crate::query::{ProjectQuery, load_queries};

#[derive(Debug, Clone)]
pub struct ProjectSnapshot {
    pub manifest: ProjectManifest,
    pub connections: Vec<crate::connection::ProjectConnection>,
    pub queries: Vec<ProjectQuery>,
    pub favorites: Vec<String>,
    pub active_environment: String,
}

pub fn load_project(project_root: &Path) -> Result<ProjectSnapshot> {
    let manifest = load_manifest(project_root)?;
    let connections = load_connections(project_root)?;
    let queries = load_queries(project_root)?;
    let favorites = load_favorites(project_root)?;
    let active_environment = load_active_environment(project_root)?.name;
    Ok(ProjectSnapshot {
        manifest,
        connections,
        queries,
        favorites,
        active_environment,
    })
}
