//! Parse and load the `.based/` project format (no UI dependencies).

mod connection;
mod env_value;
mod environment;
mod favorites;
mod load;
mod project;
mod query;
mod target;
mod walk;

pub use connection::{ConnectionSpec, PragmaSettings, ProjectConnection};
pub use environment::{ActiveEnvironment, load_active_environment, persist_active_environment};
pub use favorites::{FavoriteEntry, FavoritesFile, load_favorites, persist_favorites};
pub use load::{ProjectSnapshot, load_project};
pub use project::{ProjectManifest, ProjectSettings};
pub use query::{ProjectQuery, QueryBody};
pub use target::{ConnectionRef, QueryTarget, ResolveError, TargetConnection, resolve_target};
