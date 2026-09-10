//! GPUI Kit overlay layers (dialogs, sheets, notifications).
//!
//! `Root::new` alone does not paint these; they must be included in the child view's
//! `render` output. See <https://gpui-kit.com/component/root>.

use gpui_kit::component::Root;
use gpui_kit::{App, IntoElement, ParentElement, Window, div, prelude::*};

/// Wrap main window content with dialog, sheet, and notification layers on top.
pub fn stack_gpui_overlays(
    content: impl IntoElement,
    window: &mut Window,
    cx: &mut App,
) -> impl IntoElement {
    div()
        .size_full()
        .relative()
        .child(content)
        .children(Root::render_dialog_layer(window, cx))
        .children(Root::render_sheet_layer(window, cx))
        .children(Root::render_notification_layer(window, cx))
}
