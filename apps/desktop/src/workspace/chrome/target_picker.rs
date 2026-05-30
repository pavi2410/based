//! Modal overlay when a project query target matches multiple connections.

use based_project::ProjectQuery;
use gpui::{
    App, Entity, FontWeight, InteractiveElement, IntoElement, MouseButton, ParentElement,
    SharedString, Styled, div, px,
};
use gpui_component::{
    ActiveTheme, Sizable as _,
    button::{Button, ButtonVariants},
    h_flex,
    scroll::ScrollableElement,
    v_flex,
};

use crate::connection::ConnectionId;
use crate::connection::registry::ConnectionRegistry;
use crate::workspace::Workspace;

pub fn render_target_picker(
    query: &ProjectQuery,
    candidates: &[ConnectionId],
    registry: &Entity<ConnectionRegistry>,
    workspace: Entity<Workspace>,
    cx: &mut App,
) -> impl IntoElement {
    let overlay = cx.theme().background.opacity(0.6);
    let panel_bg = cx.theme().popover;
    let border = cx.theme().border;
    let muted = cx.theme().muted_foreground;
    let fg = cx.theme().foreground;
    let title: SharedString = query.name.clone().into();

    v_flex()
        .absolute()
        .inset_0()
        .items_center()
        .justify_center()
        .bg(overlay)
        .on_mouse_down(MouseButton::Left, {
            let ws = workspace.clone();
            move |_, _, cx| {
                ws.update(cx, |w, cx| {
                    w.cancel_pending_target_pick(cx);
                });
            }
        })
        .child(
            v_flex()
                .w(px(360.0))
                .max_h(px(420.0))
                .bg(panel_bg)
                .border_1()
                .border_color(border)
                .rounded_md()
                .overflow_hidden()
                .on_mouse_down(MouseButton::Left, |_, _, cx| cx.stop_propagation())
                .child(
                    v_flex()
                        .px_3()
                        .py_2()
                        .gap_1()
                        .border_b_1()
                        .border_color(border)
                        .child(
                            div()
                                .text_sm()
                                .font_weight(FontWeight::SEMIBOLD)
                                .text_color(fg)
                                .child("Choose connection"),
                        )
                        .child(
                            div()
                                .text_xs()
                                .text_color(muted)
                                .child(format!("Open \"{title}\"")),
                        ),
                )
                .child(v_flex().flex_1().min_h_0().overflow_y_scrollbar().children(
                    candidates.iter().enumerate().map(|(i, id)| {
                        let label = registry
                            .read(cx)
                            .get(id, cx)
                            .map(|e| e.read(cx).config.label().to_string())
                            .unwrap_or_else(|| id.0.clone());
                        let conn_id = id.clone();
                        let ws = workspace.clone();
                        h_flex()
                            .id(SharedString::from(format!("target-pick-{i}")))
                            .px_3()
                            .py_2()
                            .gap_2()
                            .items_center()
                            .border_b_1()
                            .border_color(border)
                            .cursor_pointer()
                            .child(
                                v_flex()
                                    .flex_1()
                                    .min_w_0()
                                    .child(
                                        div()
                                            .text_xs()
                                            .font_weight(FontWeight::MEDIUM)
                                            .text_color(fg)
                                            .truncate()
                                            .child(label),
                                    )
                                    .child(
                                        div()
                                            .text_xs()
                                            .text_color(muted)
                                            .truncate()
                                            .child(conn_id.0.clone()),
                                    ),
                            )
                            .on_mouse_down(MouseButton::Left, move |_, _, cx| {
                                let ws = ws.clone();
                                let id = conn_id.clone();
                                ws.update(cx, |w, cx| {
                                    w.resolve_pending_target(id, cx);
                                });
                            })
                    }),
                ))
                .child(
                    h_flex().px_3().py_2().justify_end().child(
                        Button::new("target-pick-cancel")
                            .ghost()
                            .small()
                            .label("Cancel")
                            .on_click({
                                let ws = workspace.clone();
                                move |_, _, cx| {
                                    ws.update(cx, |w, cx| {
                                        w.cancel_pending_target_pick(cx);
                                    });
                                }
                            }),
                    ),
                ),
        )
}
