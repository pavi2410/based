//! Command palette (⌘K / Ctrl+K): quick jump to connections, saved queries, and history.
//!
//! Dependency rule: may use `connection/`, `query_store/`, `workspace/{connection_tree, tab_spec,
//! project_query}`, and `widgets/`. Must not depend on engine modules or dock internals.

mod entries;
mod format;
mod render;
mod search;
mod selection;
mod types;

pub use types::{PaletteEvent, WorkspacePaletteAction};

use gpui::{App, AppContext as _, Context, Entity, EventEmitter, FocusHandle, Focusable, Window};
use gpui_component::IndexPath;
use gpui_component::command::CommandState;

use crate::connection::registry::ConnectionRegistry;
use crate::workspace::connection_tree::ConnectionTree;

use types::PaletteSection;

pub struct CommandPalette {
    registry: Entity<ConnectionRegistry>,
    connection_tree: Entity<ConnectionTree>,
    command_state: Entity<CommandState>,
    sections: Vec<PaletteSection>,
    visible: bool,
    focus_handle: FocusHandle,
}

impl CommandPalette {
    pub fn new(
        registry: Entity<ConnectionRegistry>,
        connection_tree: Entity<ConnectionTree>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Self {
        let command_state = cx.new(|cx| CommandState::new(window, cx));
        Self {
            registry,
            connection_tree,
            command_state,
            sections: vec![],
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
            self.command_state.update(cx, |state, cx| {
                state.set_query("", window, cx);
            });
            self.refresh_results(cx);
            let focus = self.command_state.read(cx).focus_handle(cx);
            focus.focus(window, cx);
        }
        cx.notify();
    }

    pub fn dismiss(&mut self, cx: &mut Context<Self>) {
        self.visible = false;
        cx.notify();
    }

    fn on_query(&mut self, query: &str, cx: &mut Context<Self>) {
        self.replace_sections(query, cx);
    }

    fn confirm(&mut self, index: IndexPath, cx: &mut Context<Self>) {
        let Some(entry) = search::item_at(&self.sections, index.section, index.row).cloned() else {
            return;
        };
        selection::emit_selection(&entry, cx);
        self.dismiss(cx);
    }

    fn refresh_results(&mut self, cx: &mut Context<Self>) {
        let query = self.command_state.read(cx).query(cx);
        self.replace_sections(query.trim(), cx);
    }

    fn replace_sections(&mut self, query: &str, cx: &mut Context<Self>) {
        self.sections = search::collect_sections(
            search::SearchContext {
                registry: &self.registry,
                connection_tree: &self.connection_tree,
            },
            query,
            cx,
        );
        cx.notify();
    }
}

impl EventEmitter<PaletteEvent> for CommandPalette {}

impl Focusable for CommandPalette {
    fn focus_handle(&self, cx: &App) -> FocusHandle {
        if self.visible {
            self.command_state.read(cx).focus_handle(cx)
        } else {
            self.focus_handle.clone()
        }
    }
}
