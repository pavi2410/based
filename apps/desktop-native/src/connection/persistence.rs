// WorkspaceState persistence — serializes open tabs, sidebar widths, and
// active-tab pointer to .based/state/tabs/<window_uuid>.json.
// Implemented in Phase 2.

use serde::{Deserialize, Serialize};

use crate::connection::TabId;

#[derive(Debug, Default, Serialize, Deserialize)]
pub struct WorkspaceState {
    pub open_tabs: Vec<TabId>,
    pub active_tab: Option<TabId>,
    pub sidebar_width_px: u32,
}
