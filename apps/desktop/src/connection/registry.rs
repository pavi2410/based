// ConnectionRegistry — Entity<ConnectionRegistry> holds every connection entry
// for the current workspace window.  Multiple windows sharing the same project
// share the same registry Entity handle; GPUI's observe/notify propagates
// state changes to all windows without IPC.

use std::collections::HashSet;

use gpui::{App, AppContext as _, Context, Entity, EventEmitter};

use super::{ConnectionEntry, ConnectionId, ConnectionOrigin, ConnectionState};

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
        self.sync_origin_entries(ConnectionOrigin::Project, entries, cx);
    }

    /// Replace connections for one origin; leave the other origin untouched.
    pub fn sync_origin_entries(
        &mut self,
        origin: ConnectionOrigin,
        entries: Vec<ConnectionEntry>,
        cx: &mut Context<Self>,
    ) {
        let new_ids: HashSet<_> = entries.iter().map(|e| e.id.clone()).collect();
        self.connections.retain(|entity| {
            let entry = entity.read(cx);
            let id = entry.id.clone();
            let keep = retain_after_origin_sync(
                origin,
                entry.origin,
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
                    e.origin = entry.origin;
                });
            } else {
                self.add(entry, cx);
            }
        }

        self.connections
            .sort_by_key(|entity| match entity.read(cx).origin {
                ConnectionOrigin::Project => 0u8,
                ConnectionOrigin::Personal => 1,
            });
    }

    pub fn ordered_ids(&self, cx: &App) -> Vec<ConnectionId> {
        self.connections
            .iter()
            .map(|e| e.read(cx).id.clone())
            .collect()
    }

    /// Remove `.based/` project connections after Close Project.
    /// Workspace-local wizard templates (`ws-template:`) stay in the registry.
    pub fn remove_project_owned(&mut self, cx: &mut Context<Self>) {
        self.connections.retain(|entity| {
            let id = entity.read(cx).id.clone();
            let keep = retain_after_project_close(&id);
            if !keep {
                cx.emit(RegistryEvent::Removed(id));
            }
            keep
        });
    }
}

/// Keep user-local workspace templates; drop `.based/` project rows.
pub(crate) fn retain_after_project_close(id: &ConnectionId) -> bool {
    id.is_workspace_local()
}

pub(crate) fn retain_after_origin_sync(
    syncing: ConnectionOrigin,
    entry_origin: ConnectionOrigin,
    id: &ConnectionId,
    is_connected: bool,
    snapshot_ids: &HashSet<ConnectionId>,
) -> bool {
    if id.is_ephemeral() {
        return true;
    }
    if entry_origin != syncing {
        return true;
    }
    if syncing == ConnectionOrigin::Project && id.is_workspace_local() {
        return true;
    }
    snapshot_ids.contains(id) || is_connected
}

/// Project reload only owns `.based/connections/` rows.
pub(crate) fn retain_after_project_sync(
    id: &ConnectionId,
    is_connected: bool,
    snapshot_ids: &HashSet<ConnectionId>,
) -> bool {
    retain_after_origin_sync(
        ConnectionOrigin::Project,
        if id.is_workspace_local() {
            ConnectionOrigin::Personal
        } else {
            ConnectionOrigin::Project
        },
        id,
        is_connected,
        snapshot_ids,
    )
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

    #[test]
    fn close_project_keeps_workspace_template() {
        let wizard = id("ws-template:aaaaaaaa-bbbb-cccc-dddd-eeeeeeeeeeee");
        assert!(retain_after_project_close(&wizard));
    }

    #[test]
    fn close_project_drops_project_connection_even_if_live() {
        let northwind = id("local/northwind");
        assert!(!retain_after_project_close(&northwind));
    }

    #[test]
    fn project_sync_keeps_personal_user_id() {
        let personal = id("user:analytics");
        let snapshot = HashSet::new();
        assert!(retain_after_project_sync(&personal, false, &snapshot));
    }

    #[test]
    fn origin_sync_keeps_other_origin_when_snapshot_empty() {
        let personal = id("user:analytics");
        let snapshot = HashSet::new();
        assert!(retain_after_origin_sync(
            ConnectionOrigin::Project,
            ConnectionOrigin::Personal,
            &personal,
            false,
            &snapshot,
        ));
    }

    #[test]
    fn close_project_keeps_personal_user_id() {
        assert!(retain_after_project_close(&id("user:analytics")));
    }

    #[test]
    fn origin_sync_keeps_unsaved_session_not_on_disk() {
        let unsaved = id("unsaved:aaaaaaaa-bbbb-cccc-dddd-eeeeeeeeeeee");
        let snapshot = HashSet::new();
        assert!(retain_after_origin_sync(
            ConnectionOrigin::Personal,
            ConnectionOrigin::Personal,
            &unsaved,
            false,
            &snapshot,
        ));
        assert!(retain_after_project_close(&unsaved));
        assert!(retain_after_project_sync(&unsaved, false, &snapshot));
    }
}
