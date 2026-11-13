//! Engine-agnostic types and local session persistence (no UI dependencies).

pub mod ai;
pub mod catalog;
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
pub use catalog::{DbColumn, DbObject, ObjectKind, Relation};
pub use connection_error::{ConnectError, ConnectionErrorCategory};
pub use connection_id::{ConnectionId, ConnectionKey, ConnectionScope};
pub use engine::{EngineKind, UnknownEngine};
pub use object::ObjectRef;
pub use page::{
    ColumnCatalog, ColumnMeta, FkTarget, Grid, PageSort, RawPage, TableExtra, TableInfo,
    page_order_sql, quote_ident,
};
pub use result::{QueryError, ResultColumn, ResultSet, SqlOutcome};
pub use secret::Secret;
pub use session::{PersistedConnection, WorkspaceState};
pub use ssh::{SshEndpoint, SshTunnelConfig};
pub use ssl::{SslMode, UnknownSslMode};
pub use value::Value;
