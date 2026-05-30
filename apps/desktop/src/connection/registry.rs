// ConnectionRegistry — Entity<ConnectionRegistry> holds every connection entry
// for the current workspace window.  Multiple windows sharing the same project
// share the same registry Entity handle; GPUI's observe/notify propagates
// state changes to all windows without IPC.

use gpui::{App, AppContext as _, Context, Entity, EventEmitter};

use super::{ConnectionEntry, ConnectionId};

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
        use std::collections::HashSet;

        let new_ids: HashSet<_> = entries.iter().map(|e| e.id.clone()).collect();
        self.connections.retain(|entity| {
            let id = entity.read(cx).id.clone();
            if new_ids.contains(&id) {
                true
            } else {
                cx.emit(RegistryEvent::Removed(id));
                false
            }
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
