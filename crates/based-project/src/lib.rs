//! Parse and load the `.based/` project format (no UI dependencies).

mod connection;
mod dotenv;
mod env_value;
mod environment;
mod favorites;
mod load;
mod project;
mod query;
mod target;
mod walk;

pub use connection::{
    ConnectionSpec, PragmaSettings, ProjectConnection, load_connections,
    load_connections_from_based_dir, slug_from_label, write_connection_file,
};
pub use dotenv::{load_env_file, secret_env_key, upsert_env_file};
pub use env_value::EnvOrString;
pub use environment::{ActiveEnvironment, load_active_environment, persist_active_environment};
pub use favorites::{FavoriteEntry, FavoritesFile, load_favorites, persist_favorites};
pub use load::{ProjectSnapshot, load_project};
pub use project::{ProjectManifest, ProjectSettings};
pub use query::{ProjectQuery, QueryBody};
pub use target::{ConnectionRef, QueryTarget, ResolveError, TargetConnection, resolve_target};
