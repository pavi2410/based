use gpui_kit::component::dock::Panel;
use gpui_kit::{App, SharedString};

/// Minimal item trait — every engine-specific panel implements this so the
/// workspace can read a tab label without knowing the concrete type.
pub trait Item: Panel {
    fn tab_label(&self, cx: &App) -> SharedString;
}
