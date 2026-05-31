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

pub use label::render_strip_tab;
pub use manager::{TabEvent, TabManager};
pub use navigation::TabNavigationHistory;
pub use open::{
    DockAreaRef, SqlInject, TabManagerRef, TabOpenQueue, WorkspaceNavQueue, WorkspaceRef,
    enqueue_open_release_notes, enqueue_open_tab, enqueue_show_home, enqueue_sql_inject,
    enqueue_toggle_left_pane, enqueue_toggle_side_pane, mark_query_tab_dirty,
    request_workspace_flush, take_sql_inject,
};
pub use session::SessionSnapshot;
pub use spec::TabSpec;
