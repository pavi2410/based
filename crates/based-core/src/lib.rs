//! Engine-agnostic types and local session persistence (no UI dependencies).

pub mod ai;
pub mod connection_error;
pub mod connection_id;
pub mod engine;
pub mod object;
pub mod page;
pub mod result;
pub mod secret;
pub mod session;
pub mod ssh;
pub mod ssl;
pub mod value;

pub use ai::ContextLevel;
pub use connection_error::{
    ConnectionErrorCategory, ConnectionErrorDetail, categorize_connect_error,
};
pub use connection_id::{ConnectionId, ConnectionKey, ConnectionScope};
pub use engine::{EngineKind, UnknownEngine};
pub use object::ObjectRef;
pub use page::{
    ColumnCatalog, ColumnMeta, FkTarget, Grid, RawPage, TableExtra, TableInfo, quote_ident,
};
pub use result::{ResultColumn, ResultSet, SqlOutcome};
pub use secret::Secret;
pub use session::{PersistedConnection, WorkspaceState};
pub use ssh::{SshEndpoint, SshTunnelConfig};
pub use ssl::{SslMode, UnknownSslMode};
pub use value::Value;
