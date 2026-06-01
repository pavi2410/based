//! Engine-agnostic types and local session persistence (no UI dependencies).

pub mod auth;
pub mod connection_error;
pub mod connection_id;
pub mod engine;
pub mod session;
pub mod tab;

pub use auth::AuthMethod;
pub use connection_error::{
    ConnectionErrorCategory, ConnectionErrorDetail, categorize_connect_error,
};
pub use connection_id::ConnectionId;
pub use engine::EngineKind;
pub use session::{PersistedConnection, WorkspaceState};
pub use tab::{TabId, TabKind};
