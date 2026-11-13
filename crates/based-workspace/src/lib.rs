//! Workspace model for P0: loose queries, collections, and user-defined environments.

pub mod ids;
pub mod model;
pub mod resolve;

pub use ids::{CollectionId, EnvironmentId, QueryId, TemplateId, WorkspaceId};
pub use model::{
    Collection, ConnectionTemplate, Environment, LooseQuery, SavedQueryRef, WorkspaceModel,
};
pub use resolve::{ResolvedConnectionTemplate, resolve_connection_template};
