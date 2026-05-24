use gpui::{
    App, Context, ElementId, Entity, FontWeight, InteractiveElement, IntoElement, ParentElement,
    RenderOnce, SharedString, Task, WeakEntity, Window, div, prelude::*, px,
};
use gpui_component::{
    ActiveTheme, Icon, IconName, IndexPath, Selectable, Sizable as _, StyledExt, h_flex,
    list::{ListDelegate, ListState},
    menu::{ContextMenuExt, PopupMenuItem},
    tooltip::Tooltip,
    v_flex,
};

use crate::connection::{ConnectionState, EngineKind};
use crate::widgets::ui::{SIDEBAR_INSET, engine_color, engine_label_inline};

use super::ConnectionTree;

#[derive(Clone)]
struct ConnectionRow {
    idx: usize,
    conn_label: SharedString,
    state_label: SharedString,
    engine: EngineKind,
    state_color: gpui::Hsla,
    is_connected: bool,
    is_failed: bool,
    fail_reason: Option<String>,
}

pub(crate) struct ConnectionListDelegate {
    tree: Entity<ConnectionTree>,
    rows: Vec<ConnectionRow>,
    all_rows: Vec<ConnectionRow>,
    query: String,
    selected_index: Option<IndexPath>,
}

impl ConnectionListDelegate {
    pub(crate) fn new(tree: Entity<ConnectionTree>) -> Self {
        Self {
            tree,
            rows: Vec::new(),
            all_rows: Vec::new(),
            query: String::new(),
            selected_index: None,
        }
    }

    pub(crate) fn sync(&mut self, all_rows: Vec<ConnectionRow>, selected: Option<usize>) {
        self.all_rows = all_rows;
        self.apply_filter();
        self.selected_index = selected.and_then(|registry_idx| {
            self.rows
                .iter()
                .position(|r| r.idx == registry_idx)
                .map(IndexPath::new)
        });
    }

    fn apply_filter(&mut self) {
        let q = self.query.trim().to_lowercase();
        if q.is_empty() {
            self.rows = self.all_rows.clone();
        } else {
            self.rows = self
                .all_rows
                .iter()
                .filter(|row| {
                    row.conn_label.to_lowercase().contains(&q)
                        || row.engine.short_label().contains(&q)
                        || row.state_label.contains(&q)
                })
                .cloned()
                .collect();
        }
    }

    fn row_at(&self, ix: IndexPath) -> Option<&ConnectionRow> {
        self.rows.get(ix.row)
    }
}

fn connection_state_dot(state: &ConnectionState, t: &gpui_component::Theme) -> gpui::Hsla {
    match state {
        ConnectionState::Disconnected => t.muted_foreground.opacity(0.75),
        ConnectionState::Connecting { .. } => t.yellow.opacity(0.95),
        ConnectionState::Connected(_) => t.green_light,
        ConnectionState::Failed { .. } => t.red,
    }
}

pub(crate) fn build_connection_rows(tree: &ConnectionTree, cx: &App) -> Vec<ConnectionRow> {
    tree.registry
        .read(cx)
        .connections()
        .iter()
        .enumerate()
        .map(|(idx, ent)| {
            let entry = ent.read(cx);
            ConnectionRow {
                idx,
                conn_label: entry.config.label().to_string().into(),
                state_label: entry.state.label().into(),
                engine: entry.config.engine(),
                state_color: connection_state_dot(&entry.state, cx.theme()),
                is_connected: matches!(entry.state, ConnectionState::Connected(_)),
                is_failed: matches!(entry.state, ConnectionState::Failed { .. }),
                fail_reason: match &entry.state {
                    ConnectionState::Failed { reason, .. } => Some(reason.clone()),
                    _ => None,
                },
            }
        })
        .collect()
}

#[derive(IntoElement)]
pub(crate) struct ConnectionRowItem {
    id: ElementId,
    row: ConnectionRow,
    selected: bool,
    tree: WeakEntity<ConnectionTree>,
}

impl Selectable for ConnectionRowItem {
    fn selected(mut self, selected: bool) -> Self {
        self.selected = selected;
        self
    }

    fn is_selected(&self) -> bool {
        self.selected
    }
}

impl RenderOnce for ConnectionRowItem {
    fn render(self, _window: &mut Window, cx: &mut App) -> impl IntoElement {
        let ConnectionRow {
            idx,
            conn_label,
            state_label,
            engine,
            state_color,
            is_connected,
            is_failed,
            fail_reason,
        } = self.row;

        let muted = cx.theme().muted_foreground;
        let sfg = cx.theme().sidebar_foreground;
        let err_fg = cx.theme().danger_foreground;
        let list_hover = cx.theme().list_hover;

        let status_cell = if is_failed {
            h_flex()
                .flex_shrink_0()
                .gap_1()
                .items_center()
                .child(
                    Icon::new(IconName::CircleX)
                        .text_color(err_fg)
                        .with_size(gpui_component::Size::XSmall),
                )
                .child(
                    div()
                        .text_xs()
                        .font_weight(FontWeight::MEDIUM)
                        .text_color(err_fg)
                        .child("Failed"),
                )
        } else {
            h_flex().flex_shrink_0().child(
                div()
                    .text_xs()
                    .text_color(muted.opacity(0.88))
                    .child(state_label),
            )
        };

        let mut row = v_flex()
            .id(self.id)
            .w_full()
            .gap(px(2.0))
            .px(px(SIDEBAR_INSET))
            .py(px(4.0))
            .cursor_pointer()
            .when(self.selected, |r| {
                r.bg(cx.theme().accent.opacity(0.15))
                    .border_l_2()
                    .border_color(engine_color(engine).opacity(0.55))
            })
            .when(is_failed && !self.selected, |r| {
                r.border_l_2().border_color(cx.theme().danger.opacity(0.5))
            })
            .hover(move |s| s.bg(list_hover))
            .child(
                h_flex()
                    .w_full()
                    .gap_2()
                    .items_center()
                    .child(
                        div()
                            .w_2()
                            .h_2()
                            .rounded_full()
                            .flex_shrink_0()
                            .bg(state_color),
                    )
                    .child(
                        div()
                            .flex_1()
                            .min_w_0()
                            .text_sm()
                            .text_color(sfg)
                            .truncate()
                            .when(is_failed, |d| d.text_color(err_fg.opacity(0.92)))
                            .child(conn_label),
                    )
                    .child(status_cell),
            )
            .child(
                h_flex()
                    .gap_1()
                    .items_center()
                    .pl(px(10.0))
                    .child(engine_label_inline(engine, cx))
                    .child(
                        div()
                            .text_xs()
                            .text_color(muted.opacity(0.78))
                            .child("local"),
                    ),
            );

        if let Some(reason) = fail_reason {
            let reason_tip: SharedString = reason.into();
            row = row.tooltip(move |window, app| {
                Tooltip::element({
                    let reason_tip = reason_tip.clone();
                    move |_w, tip_cx| {
                        let fg = tip_cx.theme().foreground;
                        let subtle = tip_cx.theme().muted_foreground;
                        v_flex()
                            .gap_1()
                            .max_w(gpui::px(400.0))
                            .child(
                                div()
                                    .text_xs()
                                    .font_semibold()
                                    .text_color(fg)
                                    .child("Could not connect"),
                            )
                            .child(
                                div()
                                    .text_xs()
                                    .text_color(subtle)
                                    .font_family(tip_cx.theme().mono_font_family.clone())
                                    .child(reason_tip.clone()),
                            )
                    }
                })
                .build(window, app)
            });
        }

        let tree = self.tree.clone();
        row.context_menu(move |menu, _window, cx| {
            if let Some(tree_ent) = tree.upgrade() {
                tree_ent.update(cx, |tree, cx| {
                    tree.selected_connection = Some(idx);
                    cx.notify();
                });
            }
            let mut menu = menu.item(PopupMenuItem::new("New Query").on_click({
                let tree = tree.clone();
                move |_, _window, cx| {
                    if let Some(tree_ent) = tree.upgrade() {
                        tree_ent.update(cx, |tree, cx| {
                            tree.open_new_query(idx, cx);
                        });
                    }
                }
            }));
            if is_connected {
                menu = menu.item(PopupMenuItem::new("Disconnect").on_click({
                    let tree = tree.clone();
                    move |_, _window, cx| {
                        if let Some(tree_ent) = tree.upgrade() {
                            tree_ent.update(cx, |tree, cx| {
                                tree.disconnect_at(idx, cx);
                            });
                        }
                    }
                }));
            }
            menu
        })
    }
}

impl ListDelegate for ConnectionListDelegate {
    type Item = ConnectionRowItem;

    fn items_count(&self, _section: usize, _: &App) -> usize {
        self.rows.len()
    }

    fn perform_search(
        &mut self,
        query: &str,
        _window: &mut Window,
        _cx: &mut Context<ListState<Self>>,
    ) -> Task<()> {
        self.query = query.to_string();
        self.apply_filter();
        Task::ready(())
    }

    fn render_item(
        &mut self,
        ix: IndexPath,
        _window: &mut Window,
        _cx: &mut Context<ListState<Self>>,
    ) -> Option<Self::Item> {
        let row = self.row_at(ix)?.clone();
        let selected = self.selected_index == Some(ix);
        Some(ConnectionRowItem {
            id: ("conn-row", row.idx).into(),
            row,
            selected,
            tree: self.tree.downgrade(),
        })
    }

    fn render_empty(
        &mut self,
        _window: &mut Window,
        cx: &mut Context<ListState<Self>>,
    ) -> impl IntoElement {
        v_flex()
            .flex_1()
            .items_center()
            .justify_center()
            .p_3()
            .child(
                div()
                    .text_xs()
                    .text_color(cx.theme().muted_foreground)
                    .child("No connections match your search."),
            )
    }

    fn set_selected_index(
        &mut self,
        ix: Option<IndexPath>,
        _window: &mut Window,
        cx: &mut Context<ListState<Self>>,
    ) {
        self.selected_index = ix;
        let selected = ix.and_then(|i| self.row_at(i).map(|r| r.idx));
        self.tree.update(cx, |tree, cx| {
            tree.selected_connection = selected;
            cx.notify();
        });
        cx.notify();
    }

    fn confirm(
        &mut self,
        _secondary: bool,
        window: &mut Window,
        cx: &mut Context<ListState<Self>>,
    ) {
        let Some(ix) = self.selected_index else {
            return;
        };
        let Some(idx) = self.row_at(ix).map(|r| r.idx) else {
            return;
        };
        self.tree.update(cx, |tree, cx| {
            tree.on_connection_row_clicked(idx, window, cx);
        });
    }
}

pub(crate) fn ensure_connection_list(
    tree: &mut ConnectionTree,
    window: &mut Window,
    cx: &mut Context<ConnectionTree>,
) -> Entity<ListState<ConnectionListDelegate>> {
    if let Some(list) = tree.connection_list.clone() {
        return list;
    }

    let tree_entity = cx.entity();
    let delegate = ConnectionListDelegate::new(tree_entity);
    let list = cx.new(|cx| {
        ListState::new(delegate, window, cx)
            .searchable(true)
            .selectable(true)
    });

    tree.connection_list = Some(list.clone());
    list
}

pub(crate) fn refresh_connection_list(tree: &ConnectionTree, cx: &mut Context<ConnectionTree>) {
    let Some(list) = tree.connection_list.clone() else {
        return;
    };
    let rows = build_connection_rows(tree, cx);
    let selected = tree.selected_connection;
    list.update(cx, |list, cx| {
        list.delegate_mut().sync(rows, selected);
        cx.notify();
    });
}
