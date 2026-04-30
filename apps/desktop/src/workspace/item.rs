use gpui::{App, SharedString};
use gpui_component::dock::Panel;

/// Minimal item trait — every engine-specific panel implements this so the
/// workspace can read a tab label without knowing the concrete type.
pub trait Item: Panel {
    fn tab_label(&self, cx: &App) -> SharedString;
}
