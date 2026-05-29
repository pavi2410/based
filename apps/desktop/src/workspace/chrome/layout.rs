//! Main workspace shell layout: connection rail | center dock | optional side pane.

use gpui::{AnyElement, App, Entity, IntoElement, ParentElement, div, prelude::*, px};
use gpui_component::{ActiveTheme, dock::DockArea, h_flex, v_flex};

/// Build the workspace body: left sidebar (toggleable), center dock, optional right side pane.
pub fn render_body_row(
    sidebar_collapsed: bool,
    sidebar: impl IntoElement,
    dock_area: Entity<DockArea>,
    side_pane: Option<AnyElement>,
    cx: &App,
) -> impl IntoElement {
    let border = cx.theme().sidebar_border;
    let sidebar_bg = cx.theme().sidebar;

    let sidebar = v_flex()
        .w(px(274.0))
        .h_full()
        .min_h_0()
        .flex_shrink_0()
        .overflow_hidden()
        .border_r_1()
        .border_color(border)
        .bg(sidebar_bg)
        .child(sidebar);

    let dock_host = div()
        .flex_1()
        .size_full()
        .overflow_hidden()
        .child(dock_area);

    let row = h_flex().flex_1().overflow_hidden();

    let row = if sidebar_collapsed {
        row.child(dock_host)
    } else {
        row.child(sidebar).child(dock_host)
    };

    if let Some(pane) = side_pane {
        row.child(pane)
    } else {
        row
    }
}
