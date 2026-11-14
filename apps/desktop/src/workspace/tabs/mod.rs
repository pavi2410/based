//! Tab lifecycle: spec, manager, labels, dispatch, commands, navigation, queuing, session.

pub mod close;
pub mod commands;
pub mod dispatch;
pub mod inbox;
pub mod infer;
pub mod kind;
pub mod label;
pub mod manager;
pub mod menu;
pub mod navigation;
pub mod open;
pub mod session;
pub mod spec;

pub use close::{CloseOutcome, ClosePrompt, CloseScope};
pub use dispatch::OpenOutcome;
pub use inbox::{Inbox, Request, request_workspace_flush};
pub use kind::PanelKind;
pub use label::render_strip_tab;
pub use manager::{TabEvent, TabId, TabManager};
pub use navigation::TabNavigationHistory;
pub use open::{Workspaces, clear_tab_dirty, mark_tab_dirty, note_tab_edited};
pub use session::{SessionScope, SessionSnapshot, SessionState, SessionTab};
pub use spec::{DrillFilter, QueryEditorInit, TabSpec};
