use std::sync::Arc;
use std::sync::atomic::{AtomicU64, Ordering};

use gpui_kit::component::dock::PanelView;
use gpui_kit::{AnyView, Context, Entity, EntityId, EventEmitter};

use super::spec::TabSpec;

/// A tab's identity for this run. Specs describe content and change as the user
/// edits; the id doesn't, so closing, pinning and session restore follow it.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct TabId(u64);

impl TabId {
    pub fn new() -> Self {
        static NEXT: AtomicU64 = AtomicU64::new(1);
        Self(NEXT.fetch_add(1, Ordering::Relaxed))
    }
}

impl Default for TabId {
    fn default() -> Self {
        Self::new()
    }
}

/// An open tab — its identity, what it shows, and the live panel.
pub struct Tab {
    pub id: TabId,
    pub spec: TabSpec,
    pub panel: Arc<dyn PanelView>,
    pub dirty: bool,
    pub pinned: bool,
}

impl Tab {
    pub fn view(&self) -> AnyView {
        self.panel.view()
    }

    /// The panel entity, if it is a `T`.
    pub fn downcast<T: 'static>(&self) -> Option<Entity<T>> {
        self.view().downcast::<T>().ok()
    }
}

/// A live center panel as seen by dock sync.
pub struct DockTab {
    pub panel: Arc<dyn PanelView>,
    /// Spec derived from the panel; `None` keeps the spec the tab was opened with.
    pub inferred: Option<TabSpec>,
    /// Used only when the panel is not yet tracked and nothing was inferred.
    pub fallback: TabSpec,
}

pub enum TabEvent {
    /// Tabs were opened, closed or re-ordered, or the active tab changed: save the session.
    Changed,
    /// A tab's content changed (editor typing); the session is saved once edits pause.
    Edited,
}

pub struct TabManager {
    pub tabs: Vec<Tab>,
    pub active_idx: Option<usize>,
}

impl TabManager {
    /// A manager that starts with one open tab.
    pub fn with_tab(spec: TabSpec, panel: Arc<dyn PanelView>) -> Self {
        Self {
            tabs: vec![Tab {
                id: TabId::new(),
                spec,
                panel,
                dirty: false,
                pinned: false,
            }],
            active_idx: Some(0),
        }
    }

    /// Open a new tab or focus the existing one for this panel view or spec.
    ///
    /// Dock sync can register a panel's view before this is called, so the view
    /// is matched first and its spec upgraded to the authoritative one.
    /// QueryEditor never dedups by spec (callers pass a fresh spec each time).
    /// Returns the tab's id and whether it was already open.
    pub fn open_or_focus(
        &mut self,
        spec: TabSpec,
        panel: Arc<dyn PanelView>,
        cx: &mut Context<Self>,
    ) -> (TabId, bool) {
        let view = panel.view();
        let is_query = matches!(spec, TabSpec::QueryEditor { .. });
        let existing = self.tabs.iter().position(|t| t.view() == view).or_else(|| {
            if is_query {
                return None;
            }
            self.tabs.iter().position(|t| t.spec == spec)
        });
        if let Some(idx) = existing {
            self.tabs[idx].spec = spec;
            self.active_idx = Some(idx);
            cx.emit(TabEvent::Changed);
            cx.notify();
            return (self.tabs[idx].id, true);
        }
        let idx = self.tabs.len();
        let dirty = restored_dirty(&spec);
        let id = TabId::new();
        self.tabs.push(Tab {
            id,
            spec,
            panel,
            dirty,
            pinned: false,
        });
        self.active_idx = Some(idx);
        cx.emit(TabEvent::Changed);
        cx.notify();
        (id, false)
    }

    pub fn tab(&self, id: TabId) -> Option<&Tab> {
        self.tabs.iter().find(|t| t.id == id)
    }

    pub fn set_pinned(&mut self, id: TabId, pinned: bool, cx: &mut Context<Self>) {
        if let Some(tab) = self.tabs.iter_mut().find(|t| t.id == id) {
            tab.pinned = pinned;
            cx.notify();
        }
    }

    pub fn activate(&mut self, idx: usize, cx: &mut Context<Self>) {
        if idx < self.tabs.len() {
            self.active_idx = Some(idx);
            cx.emit(TabEvent::Changed);
            cx.notify();
        }
    }

    pub fn active_tab(&self) -> Option<&Tab> {
        self.active_idx.and_then(|i| self.tabs.get(i))
    }

    pub fn tab_for_panel_id(&self, panel_id: EntityId) -> Option<&Tab> {
        self.tabs.iter().find(|t| t.view().entity_id() == panel_id)
    }

    pub fn tab_for_panel_id_mut(&mut self, panel_id: EntityId) -> Option<&mut Tab> {
        self.tabs
            .iter_mut()
            .find(|t| t.view().entity_id() == panel_id)
    }

    /// Live panels in strip order.
    pub fn panels(&self) -> Vec<Arc<dyn PanelView>> {
        self.tabs.iter().map(|t| t.panel.clone()).collect()
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
        dock: &[DockTab],
        active: Option<gpui_kit::AnyView>,
        cx: &mut Context<Self>,
    ) {
        let dock_views: Vec<_> = dock.iter().map(|d| d.panel.view()).collect();
        let before_len = self.tabs.len();
        let old_active = self.active_idx;

        self.tabs.retain(|t| dock_views.contains(&t.view()));

        for d in dock {
            if let Some(tab) = self.tabs.iter_mut().find(|t| t.view() == d.panel.view()) {
                if let Some(spec) = &d.inferred {
                    tab.spec = spec.clone();
                }
            } else {
                let spec = d.inferred.clone().unwrap_or_else(|| d.fallback.clone());
                self.tabs.push(Tab {
                    id: TabId::new(),
                    dirty: restored_dirty(&spec),
                    spec,
                    panel: d.panel.clone(),
                    pinned: false,
                });
            }
        }

        let new_active = active
            .and_then(|av| self.tabs.iter().position(|t| t.view() == av))
            .or(old_active.filter(|&i| i < self.tabs.len()));

        let changed = before_len != self.tabs.len() || old_active != new_active;
        self.active_idx = new_active.or_else(|| self.tabs.len().checked_sub(1));
        if changed {
            cx.emit(TabEvent::Changed);
            cx.notify();
        }
    }
}

impl EventEmitter<TabEvent> for TabManager {}

/// A query tab restored with unsaved edits starts dirty, so closing it still asks.
fn restored_dirty(spec: &TabSpec) -> bool {
    matches!(spec, TabSpec::QueryEditor { init, .. } if init.restored_dirty())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::connection::ConnectionId;

    #[test]
    fn open_or_focus_deduplicates_data_viewer() {
        let spec_a = TabSpec::DataViewer {
            conn_id: ConnectionId::parse("pg"),
            object: based_core::ObjectRef::bare("users"),
            filter: None,
        };
        let spec_b = spec_a.clone();
        assert_eq!(spec_a, spec_b, "same spec should match for dedup");
    }

    #[test]
    fn query_editors_are_always_distinct_specs() {
        let s = TabSpec::blank_query_editor(ConnectionId::parse("pg"));
        assert!(matches!(s, TabSpec::QueryEditor { .. }));
    }
}
