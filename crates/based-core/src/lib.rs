//! Engine-agnostic types and local session persistence (no UI dependencies).

pub mod auth;
pub mod connection_error;
pub mod connection_id;
pub mod engine;
pub mod object;
pub mod page;
pub mod session;
pub mod ssh;
pub mod tab;
pub mod value;

pub use auth::AuthMethod;
pub use connection_error::{
    ConnectionErrorCategory, ConnectionErrorDetail, categorize_connect_error,
};
pub use connection_id::{ConnectionId, ConnectionKey, ConnectionScope};
pub use engine::{EngineKind, UnknownEngine};
pub use object::ObjectRef;
pub use page::{
    ColumnCatalog, ColumnMeta, FkTarget, Grid, RawPage, TableExtra, TableInfo, quote_ident,
};
pub use session::{PersistedConnection, WorkspaceState};
pub use ssh::SshTunnelConfig;
pub use tab::{TabId, TabKind};
pub use value::Value;
