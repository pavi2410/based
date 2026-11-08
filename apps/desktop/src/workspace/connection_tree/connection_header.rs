//! Header of the catalog pane: which connection is focused, its state, and the actions for it.

use gpui_kit::component::{
    ActiveTheme, Icon, IconName, Sizable as _,
    button::{Button, ButtonVariants},
    h_flex,
    menu::DropdownMenu,
    v_flex,
};
use gpui_kit::{
    AnyElement, App, FontWeight, IntoElement, ParentElement, SharedString, WeakEntity, div,
    prelude::*, px,
};

use crate::widgets::{SIDEBAR_INSET, engine_icon};
use crate::workspace::tabs::WorkspaceRef;
use crate::workspace::wizard_logic::can_edit_saved_connection;

use super::ConnectionTree;
use super::connection_actions_menu;
use super::connection_list::{Link, build_connection_rows};

/// Name, state line and quick actions for the focused connection; `None` without one.
pub(crate) fn connection_header(
    tree: &ConnectionTree,
    tree_entity: WeakEntity<ConnectionTree>,
    cx: &App,
) -> Option<AnyElement> {
    let idx = tree.selected_connection?;
    let row = build_connection_rows(tree, cx)
        .into_iter()
        .find(|r| r.idx == idx)?;
    let conn_id = tree.selected_connection_id(cx)?;
    let theme = cx.theme();
    let muted = theme.muted_foreground;

    let connected = row.link == Link::Connected;
    let state: SharedString = match &row.link {
        Link::Connected => "Connected".into(),
        Link::Connecting => "Connecting…".into(),
        Link::Failed(_) => "Connection failed".into(),
        Link::Disconnected => "Not connected".into(),
    };
    let editable = can_edit_saved_connection(&conn_id);
    let engine = row.engine;

    let failure: Option<SharedString> = match &row.link {
        Link::Failed(reason) => Some(
            reason
                .as_deref()
                .map(super::notify::error_one_liner)
                .unwrap_or_else(|| "Could not connect".into()),
        ),
        _ => None,
    };
    let danger = theme.danger;
    let edit_failed_id = conn_id.clone();
    let refresh_tree = tree_entity.clone();
    let connect_tree = tree_entity.clone();
    let menu_tree = tree_entity;
    let edit_id = conn_id.clone();
    let menu_id = conn_id;

    Some(
        v_flex()
            .w_full()
            .gap(px(4.0))
            .px(px(SIDEBAR_INSET))
            .py(px(8.0))
            .border_b_1()
            .border_color(theme.sidebar_border)
            .child(
                h_flex()
                    .gap_2()
                    .items_center()
                    .child(engine_icon(engine))
                    .child(
                        div()
                            .flex_1()
                            .min_w_0()
                            .text_sm()
                            .font_weight(FontWeight::SEMIBOLD)
                            .truncate()
                            .child(row.conn_label.clone()),
                    )
                    .when(connected, |h| {
                        h.child(
                            Button::new("conn-header-refresh")
                                .ghost()
                                .xsmall()
                                .icon(Icon::new(IconName::RefreshCw))
                                .tooltip("Refresh catalog")
                                .on_click(move |_, _, cx| {
                                    if let Some(ent) = refresh_tree.upgrade() {
                                        ent.update(cx, |t, cx| t.refresh_connection(idx, cx));
                                    }
                                }),
                        )
                    })
                    .when(editable, |h| {
                        h.child(
                            Button::new("conn-header-edit")
                                .ghost()
                                .xsmall()
                                .icon(Icon::new(IconName::Settings))
                                .tooltip("Edit connection")
                                .on_click(move |_, window, cx| {
                                    if let Some(ws) =
                                        cx.try_global::<WorkspaceRef>().map(|w| w.0.clone())
                                    {
                                        ws.update(cx, |ws, cx| {
                                            ws.open_edit_connection_tab(
                                                edit_id.clone(),
                                                window,
                                                cx,
                                            );
                                        });
                                    }
                                }),
                        )
                    })
                    .child(
                        Button::new("conn-header-more")
                            .ghost()
                            .xsmall()
                            .icon(Icon::new(IconName::Ellipsis))
                            .tooltip("More")
                            .dropdown_menu(move |menu, _window, _cx| {
                                connection_actions_menu(
                                    menu_id.clone(),
                                    engine,
                                    connected,
                                    menu_tree.clone(),
                                    menu,
                                    false,
                                )
                            }),
                    ),
            )
            .child(
                h_flex()
                    .gap_2()
                    .items_center()
                    .child(div().size(px(7.0)).rounded_full().bg(row.state_color))
                    .child(div().text_xs().text_color(muted).child(state))
                    .when(!connected && row.link != Link::Connecting, |h| {
                        h.child(div().flex_1()).child(
                            Button::new("conn-header-connect")
                                .xsmall()
                                .label(if matches!(row.link, Link::Failed(_)) {
                                    "Retry"
                                } else {
                                    "Connect"
                                })
                                .on_click(move |_, window, cx| {
                                    if let Some(ent) = connect_tree.upgrade() {
                                        ent.update(cx, |t, cx| {
                                            t.on_connection_row_clicked(idx, window, cx)
                                        });
                                    }
                                }),
                        )
                    }),
            )
            .when_some(failure, |v, message| {
                v.child(
                    v_flex()
                        .gap(px(6.0))
                        .mt(px(6.0))
                        .p(px(8.0))
                        .rounded(px(8.0))
                        .border_1()
                        .border_color(danger.opacity(0.35))
                        .bg(danger.opacity(0.08))
                        .child(
                            div()
                                .text_xs()
                                .font_weight(FontWeight::SEMIBOLD)
                                .child("Can't connect"),
                        )
                        .child(div().text_xs().text_color(muted).child(message))
                        .when(editable, |v| {
                            v.child(
                                h_flex().child(
                                    Button::new("conn-header-fix")
                                        .xsmall()
                                        .outline()
                                        .label("Edit connection")
                                        .on_click(move |_, window, cx| {
                                            if let Some(ws) =
                                                cx.try_global::<WorkspaceRef>().map(|w| w.0.clone())
                                            {
                                                ws.update(cx, |ws, cx| {
                                                    ws.open_edit_connection_tab(
                                                        edit_failed_id.clone(),
                                                        window,
                                                        cx,
                                                    );
                                                });
                                            }
                                        }),
                                ),
                            )
                        }),
                )
            })
            .into_any_element(),
    )
}
