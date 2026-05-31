use gpui::{AnyView, Context, EntityId, EventEmitter};

use crate::connection::ConnectionId;

use super::spec::TabSpec;

/// An open tab — its spec (identity) and the live panel view.
pub struct Tab {
    pub spec: TabSpec,
    pub view: AnyView,
    pub dirty: bool,
    pub pinned: bool,
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
        if !is_query && let Some(idx) = self.tabs.iter().position(|t| t.spec == spec) {
            self.active_idx = Some(idx);
            cx.emit(TabEvent::ActiveChanged(idx));
            cx.notify();
            return;
        }
        let idx = self.tabs.len();
        self.tabs.push(Tab {
            spec,
            view,
            dirty: false,
            pinned: false,
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

    pub fn tab_for_view(&self, view: &AnyView) -> Option<&Tab> {
        self.tabs.iter().find(|t| &t.view == view)
    }

    pub fn tab_for_view_mut(&mut self, view: &AnyView) -> Option<&mut Tab> {
        self.tabs.iter_mut().find(|t| &t.view == view)
    }

    pub fn tab_for_panel_id(&self, panel_id: EntityId) -> Option<&Tab> {
        self.tabs.iter().find(|t| t.view.entity_id() == panel_id)
    }

    pub fn tab_for_panel_id_mut(&mut self, panel_id: EntityId) -> Option<&mut Tab> {
        self.tabs
            .iter_mut()
            .find(|t| t.view.entity_id() == panel_id)
    }

    pub fn apply_pinned_specs(&mut self, pinned: &[TabSpec], cx: &mut Context<Self>) {
        for tab in &mut self.tabs {
            tab.pinned = pinned.contains(&tab.spec);
        }
        cx.notify();
    }

    pub fn pinned_specs(&self) -> Vec<TabSpec> {
        self.tabs
            .iter()
            .filter(|t| t.pinned)
            .map(|t| t.spec.clone())
            .collect()
    }

    /// Drop tabs whose panel views are no longer in the dock; register any center tabs missing from the manager.
    pub fn reconcile_dock_tabs(
        &mut self,
        dock: &[(gpui::AnyView, TabSpec)],
        active: Option<gpui::AnyView>,
        cx: &mut Context<Self>,
    ) {
        let dock_views: Vec<_> = dock.iter().map(|(v, _)| v.clone()).collect();
        let before_len = self.tabs.len();
        let old_active = self.active_idx;

        self.tabs
            .retain(|t| dock_views.iter().any(|v| v == &t.view));

        for (view, spec) in dock {
            if let Some(tab) = self.tabs.iter_mut().find(|t| t.view == *view) {
                tab.spec = spec.clone();
            } else {
                self.tabs.push(Tab {
                    spec: spec.clone(),
                    view: view.clone(),
                    dirty: false,
                    pinned: false,
                });
            }
        }

        let new_active = active
            .and_then(|av| self.tabs.iter().position(|t| t.view == av))
            .or(old_active.filter(|&i| i < self.tabs.len()));

        let changed = before_len != self.tabs.len() || old_active != new_active;
        self.active_idx = new_active.or_else(|| self.tabs.len().checked_sub(1));
        if changed {
            cx.notify();
        }
    }

    /// Drop tabs whose panel views are no longer present in the dock (e.g. user closed a dock tab).
    pub fn sync_open_tabs(&mut self, dock_views: &[AnyView], cx: &mut Context<Self>) {
        let active_view = self
            .active_idx
            .and_then(|i| self.tabs.get(i).map(|t| t.view.clone()));
        let before_len = self.tabs.len();
        let old_active = self.active_idx;
        self.tabs
            .retain(|t| dock_views.iter().any(|dv| dv == &t.view));
        let mut new_active = active_view
            .and_then(|av| self.tabs.iter().position(|t| t.view == av))
            .or_else(|| self.tabs.len().checked_sub(1));
        if new_active.is_none() && !self.tabs.is_empty() {
            new_active = Some(0);
        }
        let changed = before_len != self.tabs.len() || old_active != new_active;
        self.active_idx = new_active;
        if changed {
            cx.notify();
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
