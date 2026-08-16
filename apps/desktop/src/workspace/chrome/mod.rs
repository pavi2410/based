//! App shell chrome: title bar, status rail, GPUI overlay stack, main layout frame.
//!
//! Dependency rule: may use `widgets/`, `app/`, `bindings/`, `connection/` (types), and GPUI.
//! Must not depend on `postgres/`, `sqlite/`, `mongodb/`, or `tab_dispatch`.
//! `status_bar` may call `connection_tree` menus for the focused-connection chip.

pub mod env;
pub mod layout;
pub mod left_pane;
pub mod overlays;
pub mod panes;
pub mod side_pane;
pub mod status_bar;
pub mod target_picker;
pub mod topbar;
