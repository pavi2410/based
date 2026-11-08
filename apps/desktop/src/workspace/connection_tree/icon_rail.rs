//! Slack-style icon strip: one button per connection.

use gpui_kit::component::{
    ActiveTheme, Icon, IconName, Sizable as _,
    button::{Button, ButtonVariants},
    h_flex,
    menu::ContextMenuExt,
    tooltip::Tooltip,
    v_flex,
};
use gpui_kit::{
    AnyElement, App, FontWeight, Hsla, IntoElement, ParentElement, SharedString,
    TestSupportExt as _, WeakEntity, div, prelude::*, px,
};

use crate::connection::{ConnectionOrigin, EngineKind, Environment};
use crate::widgets::engine_icon;
use crate::widgets::motion;
use crate::workspace::WorkspaceRef;

use super::ConnectionTree;
use super::connection_list::{ConnectionRow, Link, RailItem};
use super::context_menu::connection_context_menu;

pub(crate) const ICON_RAIL_WIDTH: f32 = 48.0;

pub(crate) fn render_icon_rail(
    tree: WeakEntity<ConnectionTree>,
    selected: Option<usize>,
    content_expanded: bool,
    rows: Vec<RailItem>,
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
                .children(rows.into_iter().map(|item| match item {
                    RailItem::Divider => rail_divider(cx).into_any_element(),
                    RailItem::Connection(row) => {
                        rail_icon(row, selected, tree.clone(), cx).into_any_element()
                    }
                }))
                .child(
                    Button::new("rail-new-connection")
                        .ghost()
                        .small()
                        .icon(Icon::new(IconName::Plus).text_color(muted))
                        .tooltip("New connection")
                        .on_click(|_, window, cx| {
                            if let Some(ws) = cx.try_global::<WorkspaceRef>().map(|w| w.0.clone()) {
                                ws.update(cx, |ws, cx| ws.open_connection_picker_tab(window, cx));
                            }
                        }),
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

fn rail_divider(cx: &App) -> impl IntoElement {
    div()
        .w(px(20.0))
        .h(px(8.0))
        .flex()
        .items_center()
        .child(div().w_full().h(px(1.0)).bg(cx.theme().sidebar_border))
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
    let state_text = match &row.link {
        Link::Connected => "Connected",
        Link::Connecting => "Connecting…",
        Link::Failed(_) => "Connection failed",
        Link::Disconnected => "Not connected",
    };
    let card = HoverCardData {
        name: row.conn_label.clone(),
        state: state_text,
        dot: row.state_color,
        environment: row.environment,
        detail: row.detail.clone(),
        origin: row.origin,
        read_only: row.read_only,
        engine: row.engine,
    };
    let engine = row.engine;
    let is_connected = row.link == Link::Connected;
    let sidebar = cx.theme().sidebar;
    let dark = cx.theme().is_dark();
    // Light themes get white tiles; dark themes keep the themed look.
    let (base, hover) = if dark {
        (None, cx.theme().muted.opacity(0.42))
    } else {
        (Some(gpui_kit::white()), gpui_kit::white().opacity(0.85))
    };
    let selected_bg = cx.theme().sidebar_accent;
    let primary = cx.theme().primary;
    let outline = cx.theme().muted_foreground.opacity(0.25);

    div()
        .id(("rail-conn", idx))
        .test_support()
        .relative()
        .w(px(36.0))
        .h(px(36.0))
        .rounded(px(8.0))
        .flex()
        .items_center()
        .justify_center()
        .cursor_pointer()
        .border_1()
        .border_color(if is_selected && !dark {
            primary
        } else {
            outline
        })
        .when_some(base, |d, c| d.bg(c))
        .when(is_selected && dark, |d| d.bg(selected_bg))
        .when(!is_selected, |d| d.hover(move |d| d.bg(hover)))
        .tooltip(move |window, app| {
            let card = card.clone();
            Tooltip::element(move |_, cx| hover_card(&card, cx)).build(window, app)
        })
        .on_click(move |_, window, cx| {
            if let Some(ent) = tree_click.upgrade() {
                ent.update(cx, |t, cx| t.on_connection_row_clicked(idx, window, cx));
            }
        })
        .context_menu(move |menu, _window, cx| {
            connection_context_menu(idx, engine, is_connected, tree_menu.clone(), menu, cx)
        })
        .child(engine_icon(engine))
        .when_some(row.environment, |d, env| {
            d.child(
                div()
                    .absolute()
                    .left(px(-1.0))
                    .top(px(7.0))
                    .bottom(px(7.0))
                    .w(px(3.0))
                    .rounded_r(px(3.0))
                    .bg(env.color(cx.theme())),
            )
        })
        .child(state_ring(idx, &row.link, row.state_color, sidebar))
}

/// Status badge on the tile corner: always visible so idle, failed and busy states read at a glance.
fn state_ring(idx: usize, link: &Link, color: Hsla, ground: Hsla) -> AnyElement {
    let dot = div()
        .absolute()
        .bottom(px(2.0))
        .right(px(2.0))
        .w(px(9.0))
        .h(px(9.0))
        .rounded_full()
        .bg(color)
        .border_2()
        .border_color(ground);
    if *link == Link::Connecting {
        motion::pulse(SharedString::from(format!("rail-busy-{idx}")), dot)
    } else {
        dot.into_any_element()
    }
}

#[derive(Clone)]
struct HoverCardData {
    name: SharedString,
    state: &'static str,
    dot: Hsla,
    environment: Option<Environment>,
    detail: SharedString,
    origin: ConnectionOrigin,
    read_only: bool,
    engine: EngineKind,
}

/// Rich tooltip for a rail tile: name, environment, state and where it points.
fn hover_card(d: &HoverCardData, cx: &App) -> AnyElement {
    let theme = cx.theme();
    let muted = theme.muted_foreground;
    let fact = |k: &'static str, v: SharedString| {
        h_flex()
            .gap_3()
            .justify_between()
            .child(div().text_xs().text_color(muted).child(k))
            .child(div().text_xs().child(v))
    };
    v_flex()
        .min_w(px(220.0))
        .gap(px(6.0))
        .child(
            h_flex()
                .gap_2()
                .items_center()
                .child(engine_icon(d.engine))
                .child(
                    div()
                        .text_sm()
                        .font_weight(FontWeight::SEMIBOLD)
                        .child(d.name.clone()),
                )
                .when_some(d.environment, |h, env| {
                    let tone = env.color(theme);
                    h.child(
                        div()
                            .px(px(6.0))
                            .rounded_full()
                            .border_1()
                            .border_color(tone.opacity(0.5))
                            .bg(tone.opacity(0.12))
                            .text_color(tone)
                            .text_xs()
                            .child(env.label()),
                    )
                }),
        )
        .child(
            h_flex()
                .gap_2()
                .items_center()
                .child(div().size(px(7.0)).rounded_full().bg(d.dot))
                .child(div().text_xs().child(d.state)),
        )
        .child(fact("Where", d.detail.clone()))
        .child(fact(
            "Source",
            match d.origin {
                ConnectionOrigin::Project => "Project".into(),
                ConnectionOrigin::Personal => "Personal".into(),
            },
        ))
        .when(d.read_only, |v| v.child(fact("Access", "Read-only".into())))
        .into_any_element()
}
