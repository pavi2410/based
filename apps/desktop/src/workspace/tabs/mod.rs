//! Tab lifecycle: spec, manager, labels, dispatch, commands, navigation, queuing, session.

pub mod close;
pub mod commands;
pub mod dispatch;
pub mod inbox;
pub mod infer;
pub mod label;
pub mod manager;
pub mod menu;
pub mod navigation;
pub mod open;
pub mod session;
pub mod spec;

pub use close::{CloseOutcome, ClosePrompt, CloseScope};
pub use inbox::{Inbox, Request, request_workspace_flush};
pub use label::render_strip_tab;
pub use manager::{TabEvent, TabManager};
pub use navigation::TabNavigationHistory;
pub use open::{DockAreaRef, TabManagerRef, WorkspaceRef, mark_tab_dirty};
pub use session::{SessionSnapshot, SessionState, merge_live_and_pending};
pub use spec::{DrillFilter, QueryEditorInit, TabSpec};
