//! Main workspace shell layout: connection rail, center dock, inspector column.

use gpui::{App, Entity, IntoElement, ParentElement, div, prelude::*, px};
use gpui_component::{ActiveTheme, dock::DockArea, h_flex, v_flex};

use crate::workspace::connection_tree::ConnectionTree;

/// Sidebar rail + center dock + optional inspector column.
pub fn render_body_row(
    sidebar_collapsed: bool,
    inspector_collapsed: bool,
    connection_tree: Entity<ConnectionTree>,
    dock_area: Entity<DockArea>,
    inspector: impl IntoElement,
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
        .child(connection_tree);

    let dock_host = div()
        .flex_1()
        .size_full()
        .overflow_hidden()
        .child(dock_area);

    if sidebar_collapsed {
        h_flex()
            .flex_1()
            .overflow_hidden()
            .child(dock_host)
            .when(!inspector_collapsed, |row| row.child(inspector))
    } else {
        h_flex()
            .flex_1()
            .overflow_hidden()
            .child(sidebar)
            .child(dock_host)
            .when(!inspector_collapsed, |row| row.child(inspector))
    }
}
