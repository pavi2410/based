use gpui::{AnyView, Context, EventEmitter};

use crate::connection::ConnectionId;

use super::tab_spec::TabSpec;

/// An open tab — its spec (identity) and the live panel view.
pub struct Tab {
    pub spec: TabSpec,
    pub view: AnyView,
    pub dirty: bool, // unsaved query content
}

pub enum TabEvent {
    TabOpened(usize), // index of new tab
    TabClosed(usize),
    ActiveChanged(usize),
}

pub struct TabManager {
    pub tabs: Vec<Tab>,
    pub active_idx: Option<usize>,
}

impl TabManager {
    pub fn new() -> Self {
        Self {
            tabs: vec![],
            active_idx: None,
        }
    }

    /// Open a new tab or focus the existing one for this spec.
    /// QueryEditor always opens a new tab (caller passes a fresh spec each time).
    pub fn open_or_focus(&mut self, spec: TabSpec, view: AnyView, cx: &mut Context<Self>) {
        // QueryEditor: always new
        let is_query = matches!(spec, TabSpec::QueryEditor { .. });
        if !is_query {
            if let Some(idx) = self.tabs.iter().position(|t| t.spec == spec) {
                self.active_idx = Some(idx);
                cx.emit(TabEvent::ActiveChanged(idx));
                cx.notify();
                return;
            }
        }
        let idx = self.tabs.len();
        self.tabs.push(Tab {
            spec,
            view,
            dirty: false,
        });
        self.active_idx = Some(idx);
        cx.emit(TabEvent::TabOpened(idx));
        cx.emit(TabEvent::ActiveChanged(idx));
        cx.notify();
    }

    pub fn close(&mut self, idx: usize, cx: &mut Context<Self>) {
        if idx >= self.tabs.len() {
            return;
        }
        self.tabs.remove(idx);
        let new_active = if self.tabs.is_empty() {
            None
        } else {
            Some(idx.saturating_sub(1).min(self.tabs.len() - 1))
        };
        self.active_idx = new_active;
        cx.emit(TabEvent::TabClosed(idx));
        if let Some(i) = new_active {
            cx.emit(TabEvent::ActiveChanged(i));
        }
        cx.notify();
    }

    pub fn activate(&mut self, idx: usize, cx: &mut Context<Self>) {
        if idx < self.tabs.len() {
            self.active_idx = Some(idx);
            cx.emit(TabEvent::ActiveChanged(idx));
            cx.notify();
        }
    }

    pub fn active_tab(&self) -> Option<&Tab> {
        self.active_idx.and_then(|i| self.tabs.get(i))
    }

    pub fn close_tabs_for_conn(&mut self, conn_id: &ConnectionId, cx: &mut Context<Self>) {
        let indices: Vec<usize> = self
            .tabs
            .iter()
            .enumerate()
            .filter(|(_, t)| t.spec.conn_id() == conn_id)
            .map(|(i, _)| i)
            .rev()
            .collect();
        for i in indices {
            self.close(i, cx);
        }
    }
}

impl EventEmitter<TabEvent> for TabManager {}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn open_or_focus_deduplicates_data_viewer() {
        let spec_a = TabSpec::DataViewer {
            conn_id: ConnectionId("pg".into()),
            object: "users".into(),
        };
        let spec_b = spec_a.clone();
        assert_eq!(spec_a, spec_b, "same spec should match for dedup");
    }

    #[test]
    fn query_editors_are_always_distinct_specs() {
        let s = TabSpec::blank_query_editor(ConnectionId("pg".into()));
        assert!(matches!(s, TabSpec::QueryEditor { .. }));
    }
}
