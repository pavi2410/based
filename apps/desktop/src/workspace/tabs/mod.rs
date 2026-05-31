//! Tab lifecycle: spec, manager, labels, dispatch, commands, navigation, queuing, session.

pub mod commands;
pub mod dispatch;
pub mod infer;
pub mod label;
pub mod manager;
pub mod navigation;
pub mod open;
pub mod session;
pub mod spec;

pub use manager::{TabEvent, TabManager};
pub use navigation::TabNavigationHistory;
pub use open::{
    DockAreaRef, SqlInject, TabManagerRef, TabOpenQueue, WorkspaceNavQueue, WorkspaceRef, enqueue_open_tab, enqueue_show_home,
    enqueue_sql_inject, mark_query_tab_dirty,
};
pub use session::SessionSnapshot;
pub use spec::TabSpec;
