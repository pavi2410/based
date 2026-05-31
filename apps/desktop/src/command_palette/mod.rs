//! Command palette (⌘K / Ctrl+K): quick jump to connections, saved queries, and history.
//!
//! Dependency rule: may use `connection/`, `query_store/`, `workspace/{connection_tree, tab_spec,
//! project_query}`, and `widgets/`. Must not depend on engine modules or dock internals.

mod actions;
mod format;
mod render;
mod search;
mod selection;
mod types;

pub use actions::init;
pub use types::{PaletteEvent, PaletteResult, WorkspacePaletteAction};

use gpui::{
    App, AppContext as _, Context, Entity, EventEmitter, FocusHandle, Focusable, ScrollHandle,
    Subscription, Window, point, px,
};
use gpui_component::input::{InputEvent, InputState};

use crate::connection::registry::ConnectionRegistry;
use crate::workspace::connection_tree::ConnectionTree;

pub struct CommandPalette {
    registry: Entity<ConnectionRegistry>,
    connection_tree: Entity<ConnectionTree>,
    search_input: Entity<InputState>,
    results: Vec<PaletteResult>,
    selected: usize,
    visible: bool,
    focus_handle: FocusHandle,
    results_scroll: ScrollHandle,
    pending_scroll_to: Option<usize>,
    _search_subscription: Subscription,
}

impl CommandPalette {
    pub fn new(
        registry: Entity<ConnectionRegistry>,
        connection_tree: Entity<ConnectionTree>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Self {
        let search_input = cx
            .new(|cx| InputState::new(window, cx).placeholder("Search tables, queries, history…"));
        let _search_subscription =
            cx.subscribe_in(&search_input, window, Self::on_search_input_event);

        Self {
            registry,
            connection_tree,
            search_input,
            results: vec![],
            selected: 0,
            visible: false,
            focus_handle: cx.focus_handle(),
            results_scroll: ScrollHandle::new(),
            pending_scroll_to: None,
            _search_subscription,
        }
    }

    fn query(&self, cx: &App) -> String {
        self.search_input.read(cx).value().trim().to_string()
    }

    pub fn is_visible(&self) -> bool {
        self.visible
    }

    pub fn toggle(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.visible = !self.visible;
        if self.visible {
            self.search_input.update(cx, |input, cx| {
                input.set_value("", window, cx);
            });
            self.selected = 0;
            self.results_scroll.set_offset(point(px(0.0), px(0.0)));
            self.refresh_results(cx);
            self.search_input
                .read(cx)
                .focus_handle(cx)
                .focus(window, cx);
        }
        cx.notify();
    }

    pub fn dismiss(&mut self, cx: &mut Context<Self>) {
        self.visible = false;
        cx.notify();
    }

    fn open_selected(&mut self, secondary: bool, cx: &mut Context<Self>) {
        let Some(entry) = self.results.get(self.selected).cloned() else {
            return;
        };
        selection::emit_selection(&entry, secondary, cx);
        self.dismiss(cx);
    }

    fn on_search_input_event(
        &mut self,
        _input: &Entity<InputState>,
        event: &InputEvent,
        _window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if let InputEvent::Change = event {
            self.selected = 0;
            self.results_scroll.set_offset(point(px(0.0), px(0.0)));
            self.refresh_results(cx);
        }
    }

    fn select_prev(&mut self, cx: &mut Context<Self>) {
        if self.results.is_empty() {
            return;
        }
        self.selected = self.selected.saturating_sub(1);
        self.pending_scroll_to = Some(self.selected);
        cx.notify();
    }

    fn select_next(&mut self, cx: &mut Context<Self>) {
        if self.results.is_empty() {
            return;
        }
        let max = self.results.len() - 1;
        self.selected = (self.selected + 1).min(max);
        self.pending_scroll_to = Some(self.selected);
        cx.notify();
    }

    pub(crate) fn take_pending_scroll(&mut self) -> Option<usize> {
        self.pending_scroll_to.take()
    }

    fn refresh_results(&mut self, cx: &mut Context<Self>) {
        self.results = search::collect_results(
            search::SearchContext {
                registry: &self.registry,
                connection_tree: &self.connection_tree,
            },
            &self.query(cx),
            cx,
        );
        self.selected = self.selected.min(self.results.len().saturating_sub(1));
        cx.notify();
    }
}

impl EventEmitter<PaletteEvent> for CommandPalette {}

impl Focusable for CommandPalette {
    fn focus_handle(&self, cx: &App) -> FocusHandle {
        if self.visible {
            self.search_input.read(cx).focus_handle(cx)
        } else {
            self.focus_handle.clone()
        }
    }
}
