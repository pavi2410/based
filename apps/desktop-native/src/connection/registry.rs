// ConnectionRegistry — Entity<ConnectionRegistry> holds every connection entry
// for the current workspace window.  Multiple windows sharing the same project
// share the same registry Entity handle; GPUI's observe/notify propagates
// state changes to all windows without IPC.
// Fully implemented in Phase 2.

use std::collections::HashMap;

use super::{ConnectionEntry, ConnectionId};

pub struct ConnectionRegistry {
    entries: HashMap<ConnectionId, gpui::Entity<ConnectionEntry>>,
    order: Vec<ConnectionId>,
}

impl ConnectionRegistry {
    pub fn new() -> Self {
        Self {
            entries: HashMap::new(),
            order: Vec::new(),
        }
    }

    pub fn ordered_ids(&self) -> &[ConnectionId] {
        &self.order
    }

    pub fn get(&self, id: &ConnectionId) -> Option<&gpui::Entity<ConnectionEntry>> {
        self.entries.get(id)
    }
}

impl Default for ConnectionRegistry {
    fn default() -> Self {
        Self::new()
    }
}
