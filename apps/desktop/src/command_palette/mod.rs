//! Command palette (⌘K / Ctrl+K): quick jump to connections, saved queries, and history.

use std::ops::DerefMut;

use gpui::{
    App, Context, Entity, FocusHandle, Focusable, IntoElement, MouseButton, Render, SharedString,
    Window, div, prelude::*,
};
use gpui_component::{ActiveTheme, h_flex, scroll::ScrollableElement, v_flex};

use crate::connection::registry::ConnectionRegistry;
use crate::query_store::QueryStore;
use crate::workspace::tab_spec::TabSpec;

/// A search result the palette can return.
#[derive(Clone)]
#[allow(dead_code)]
pub struct PaletteResult {
    pub kind: ResultKind,
    pub label: String,
    pub sublabel: String,
    pub conn_label: String,
    pub spec: TabSpec,
}

#[derive(Clone, PartialEq, Eq)]
pub enum ResultKind {
    SchemaObject,
    SavedQuery,
    History,
}

pub struct CommandPalette {
    registry: Entity<ConnectionRegistry>,
    query: String,
    results: Vec<PaletteResult>,
    selected: usize,
    visible: bool,
    focus_handle: FocusHandle,
}

impl CommandPalette {
    pub fn new(registry: Entity<ConnectionRegistry>, cx: &mut Context<Self>) -> Self {
        Self {
            registry,
            query: String::new(),
            results: vec![],
            selected: 0,
            visible: false,
            focus_handle: cx.focus_handle(),
        }
    }

    pub fn is_visible(&self) -> bool {
        self.visible
    }

    pub fn toggle(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.visible = !self.visible;
        if self.visible {
            self.query.clear();
            self.refresh_results(cx);
            self.focus_handle.focus(window, cx.deref_mut());
        }
        cx.notify();
    }

    pub fn dismiss(&mut self, cx: &mut Context<Self>) {
        self.visible = false;
        cx.notify();
    }

    fn refresh_results(&mut self, cx: &mut Context<Self>) {
        let q = self.query.to_lowercase();
        let mut results = vec![];

        let ids = self.registry.read(cx).ordered_ids(cx);
        for conn_id in &ids {
            if self.registry.read(cx).get(conn_id, cx).is_some() {
                if q.is_empty() || conn_id.0.to_lowercase().contains(&q) {
                    results.push(PaletteResult {
                        kind: ResultKind::SchemaObject,
                        label: conn_id.0.clone(),
                        sublabel: String::new(),
                        conn_label: conn_id.0.clone(),
                        spec: TabSpec::Dashboard(conn_id.clone()),
                    });
                }
            }
        }

        let store = cx.global::<QueryStore>();
        for saved in store.all_saved() {
            if q.is_empty() || saved.name.to_lowercase().contains(&q) {
                results.push(PaletteResult {
                    kind: ResultKind::SavedQuery,
                    label: saved.name.clone(),
                    sublabel: saved.query_text().chars().take(60).collect(),
                    conn_label: saved.connection.0.clone(),
                    spec: TabSpec::QueryEditor(saved.connection.clone()),
                });
            }
        }

        for entry in store.history.recent(100) {
            if q.is_empty() || entry.query.to_lowercase().contains(&q) {
                results.push(PaletteResult {
                    kind: ResultKind::History,
                    label: entry.query.chars().take(72).collect(),
                    sublabel: String::new(),
                    conn_label: entry.conn_id.0.clone(),
                    spec: TabSpec::QueryEditor(entry.conn_id.clone()),
                });
            }
        }

        self.results = results;
        self.selected = self.selected.min(self.results.len().saturating_sub(1));
        cx.notify();
    }
}

impl Focusable for CommandPalette {
    fn focus_handle(&self, _: &App) -> FocusHandle {
        self.focus_handle.clone()
    }
}

impl Render for CommandPalette {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        if !self.visible {
            return div().into_any_element();
        }

        let theme = cx.theme();

        div()
            .absolute()
            .inset_0()
            .bg(gpui::rgba(0x00000088))
            .on_mouse_down(
                MouseButton::Left,
                cx.listener(|this, _, _, cx| {
                    this.dismiss(cx);
                }),
            )
            .child(
                div()
                    .absolute()
                    .top(gpui::px(120.0))
                    .left_1_2()
                    .ml(gpui::px(-280.0))
                    .w(gpui::px(560.0))
                    .track_focus(&self.focus_handle)
                    .on_mouse_down(
                        MouseButton::Left,
                        cx.listener(|_, _, _, cx| {
                            cx.stop_propagation();
                        }),
                    )
                    .bg(theme.popover)
                    .border_1()
                    .border_color(theme.border)
                    .rounded_lg()
                    .shadow_lg()
                    .child(
                        h_flex()
                            .p_3()
                            .gap_2()
                            .border_b_1()
                            .border_color(theme.border)
                            .child(div().text_color(theme.muted_foreground).child("⌕"))
                            .child(div().flex_1().text_sm().text_color(theme.foreground).child(
                                if self.query.is_empty() {
                                    SharedString::from("Search tables, queries, connections…")
                                } else {
                                    SharedString::from(self.query.clone())
                                },
                            )),
                    )
                    .child(
                        v_flex()
                            .max_h(gpui::px(360.0))
                            .overflow_y_scrollbar()
                            .children(self.results.iter().enumerate().map(|(i, r)| {
                                let is_sel = i == self.selected;
                                div()
                                    .px_3()
                                    .py_2()
                                    .flex()
                                    .gap_2()
                                    .when(is_sel, |d| d.bg(theme.accent))
                                    .child(
                                        div()
                                            .text_xs()
                                            .text_color(theme.muted_foreground)
                                            .child(r.conn_label.clone()),
                                    )
                                    .child(div().flex_1().text_sm().child(r.label.clone()))
                            })),
                    )
                    .child(
                        h_flex()
                            .px_3()
                            .py_2()
                            .border_t_1()
                            .border_color(theme.border)
                            .gap_3()
                            .text_xs()
                            .text_color(theme.muted_foreground)
                            .child("↑↓ navigate")
                            .child("↵ open")
                            .child("esc dismiss"),
                    ),
            )
            .into_any_element()
    }
}
