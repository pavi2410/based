// ConnectionRegistry — Entity<ConnectionRegistry> holds every connection entry
// for the current workspace window.  Multiple windows sharing the same project
// share the same registry Entity handle; GPUI's observe/notify propagates
// state changes to all windows without IPC.

use std::collections::HashSet;

use gpui::{App, AppContext as _, Context, Entity, EventEmitter};

use super::{ConnectionEntry, ConnectionId, ConnectionState};

/// Project reload only owns `.based/connections/` rows.
///
/// Keep wizard templates (`ws-template:`) and live sessions even when the
/// snapshot skipped them (missing env vars) or never listed them.
pub(crate) fn retain_after_project_sync(
    id: &ConnectionId,
    is_connected: bool,
    snapshot_ids: &HashSet<ConnectionId>,
) -> bool {
    snapshot_ids.contains(id) || is_connected || id.0.starts_with("ws-template:")
}

pub enum RegistryEvent {
    Added(ConnectionId),
    Removed(ConnectionId),
    StateChanged(ConnectionId),
}

pub struct ConnectionRegistry {
    connections: Vec<Entity<ConnectionEntry>>,
}

impl ConnectionRegistry {
    pub fn new(_cx: &mut Context<Self>) -> Self {
        Self {
            connections: vec![],
        }
    }

    pub fn add(
        &mut self,
        entry: ConnectionEntry,
        cx: &mut Context<Self>,
    ) -> Entity<ConnectionEntry> {
        let entity = cx.new(|_| entry);
        self.connections.push(entity.clone());
        cx.emit(RegistryEvent::Added(entity.read(cx).id.clone()));
        entity
    }

    pub fn remove(&mut self, id: &ConnectionId, cx: &mut Context<Self>) {
        if let Some(pos) = self.connections.iter().position(|e| e.read(cx).id == *id) {
            let entity = self.connections.remove(pos);
            cx.emit(RegistryEvent::Removed(entity.read(cx).id.clone()));
        }
    }

    pub fn connections(&self) -> &[Entity<ConnectionEntry>] {
        &self.connections
    }

    pub fn get(&self, id: &ConnectionId, cx: &App) -> Option<&Entity<ConnectionEntry>> {
        self.connections.iter().find(|e| e.read(cx).id == *id)
    }

    pub fn sync_project_entries(&mut self, entries: Vec<ConnectionEntry>, cx: &mut Context<Self>) {
        let new_ids: HashSet<_> = entries.iter().map(|e| e.id.clone()).collect();
        self.connections.retain(|entity| {
            let entry = entity.read(cx);
            let id = entry.id.clone();
            let keep = retain_after_project_sync(
                &id,
                matches!(entry.state, ConnectionState::Connected(_)),
                &new_ids,
            );
            if !keep {
                cx.emit(RegistryEvent::Removed(id));
            }
            keep
        });

        for entry in entries {
            if let Some(existing) = self.get(&entry.id, cx).cloned() {
                existing.update(cx, |e, _| {
                    e.config = entry.config;
                    e.tags = entry.tags;
                });
            } else {
                self.add(entry, cx);
            }
        }
    }

    pub fn ordered_ids(&self, cx: &App) -> Vec<ConnectionId> {
        self.connections
            .iter()
            .map(|e| e.read(cx).id.clone())
            .collect()
    }
}

impl EventEmitter<RegistryEvent> for ConnectionRegistry {}

#[cfg(test)]
mod tests {
    use super::*;

    fn id(key: &str) -> ConnectionId {
        ConnectionId::from_key(key)
    }

    #[test]
    fn project_sync_keeps_snapshot_row() {
        let northwind = id("local/northwind");
        let snapshot = HashSet::from([northwind.clone()]);
        assert!(retain_after_project_sync(&northwind, false, &snapshot));
    }

    #[test]
    fn project_sync_drops_disconnected_row_removed_from_disk() {
        let gone = id("local/gone");
        let snapshot = HashSet::new();
        assert!(!retain_after_project_sync(&gone, false, &snapshot));
    }

    #[test]
    fn project_sync_keeps_wizard_template_not_in_snapshot() {
        let wizard = id("ws-template:aaaaaaaa-bbbb-cccc-dddd-eeeeeeeeeeee");
        let snapshot = HashSet::new();
        assert!(retain_after_project_sync(&wizard, false, &snapshot));
    }

    #[test]
    fn project_sync_keeps_connected_row_skipped_on_reload() {
        let local_pg = id("local/postgres");
        let snapshot = HashSet::new();
        assert!(retain_after_project_sync(&local_pg, true, &snapshot));
    }
}
