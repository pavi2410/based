//! Slack-style icon strip: one button per connection.

use gpui::{
    AnyElement, App, IntoElement, MouseButton, ParentElement, WeakEntity, div, prelude::*, px,
};
use gpui_component::{
    ActiveTheme, Icon, IconName, Sizable as _,
    button::{Button, ButtonVariants},
    menu::ContextMenuExt,
    tooltip::Tooltip,
    v_flex,
};

use crate::widgets::engine_icon;

use super::ConnectionTree;
use super::connection_list::ConnectionRow;
use super::context_menu::connection_context_menu;

pub(crate) const ICON_RAIL_WIDTH: f32 = 48.0;

pub(crate) fn render_icon_rail(
    tree: WeakEntity<ConnectionTree>,
    selected: Option<usize>,
    content_expanded: bool,
    rows: Vec<ConnectionRow>,
    cx: &App,
) -> AnyElement {
    let tree_toggle = tree.clone();
    let muted = cx.theme().muted_foreground;

    v_flex()
        .w(px(ICON_RAIL_WIDTH))
        .h_full()
        .flex_shrink_0()
        .items_center()
        .border_r_1()
        .border_color(cx.theme().sidebar_border)
        .child(
            v_flex()
                .flex_1()
                .w_full()
                .items_center()
                .gap(px(4.0))
                .py(px(8.0))
                .children(
                    rows.into_iter()
                        .map(|row| rail_icon(row, selected, tree.clone(), cx)),
                ),
        )
        .child(
            Button::new("content-rail-toggle")
                .ghost()
                .small()
                .mb(px(8.0))
                .icon(
                    Icon::new(if content_expanded {
                        IconName::ChevronLeft
                    } else {
                        IconName::ChevronRight
                    })
                    .text_color(muted),
                )
                .tooltip(if content_expanded {
                    "Hide catalog"
                } else {
                    "Show catalog"
                })
                .on_click(move |_, _, cx| {
                    if let Some(ent) = tree_toggle.upgrade() {
                        ent.update(cx, |t, cx| t.toggle_content_rail(cx));
                    }
                }),
        )
        .into_any_element()
}

fn rail_icon(
    row: ConnectionRow,
    selected: Option<usize>,
    tree: WeakEntity<ConnectionTree>,
    cx: &App,
) -> impl IntoElement {
    let idx = row.idx;
    let is_selected = selected == Some(idx);
    let tree_click = tree.clone();
    let tree_menu = tree;
    let label = row.conn_label.clone();
    let engine = row.engine;
    let is_connected = row.is_connected;
    let sidebar = cx.theme().sidebar;
    let hover = cx.theme().muted.opacity(0.42);
    let outline = cx.theme().muted_foreground.opacity(0.25);

    div()
        .id(("rail-conn", idx))
        .relative()
        .w(px(36.0))
        .h(px(36.0))
        .rounded(px(8.0))
        .flex()
        .items_center()
        .justify_center()
        .cursor_pointer()
        .border_1()
        .border_color(outline)
        .when(is_selected, |d| d.bg(cx.theme().sidebar_accent))
        .when(!is_selected, |d| d.hover(move |d| d.bg(hover)))
        .tooltip(move |window, app| Tooltip::new(label.clone()).build(window, app))
        .on_mouse_down(MouseButton::Left, move |_, window, cx| {
            if let Some(ent) = tree_click.upgrade() {
                ent.update(cx, |t, cx| t.on_connection_row_clicked(idx, window, cx));
            }
        })
        .context_menu(move |menu, _window, cx| {
            connection_context_menu(idx, engine, is_connected, tree_menu.clone(), menu, cx)
        })
        .child(engine_icon(engine))
        .child(
            div()
                .absolute()
                .bottom(px(3.0))
                .right(px(3.0))
                .w(px(7.0))
                .h(px(7.0))
                .rounded_full()
                .bg(row.state_color)
                .border_1()
                .border_color(sidebar),
        )
}
