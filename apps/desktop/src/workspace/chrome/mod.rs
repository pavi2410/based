//! App shell chrome: title bar, status rail, GPUI overlay stack, main layout frame.
//!
//! Dependency rule: may use `widgets/`, `app/`, `bindings/`, `connection/` (types), and GPUI.
//! Must not depend on `postgres/`, `sqlite/`, `mongodb/`, `tab_dispatch`, or `connection_tree/`.

pub mod env;
pub mod layout;
pub mod left_pane;
pub mod overlays;
pub mod panes;
pub mod side_pane;
pub mod status_bar;
pub mod topbar;
