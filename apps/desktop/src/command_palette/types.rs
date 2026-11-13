use crate::connection::ConnectionId;
use crate::workspace::TabSpec;

/// Emitted when the user picks a palette row — workspace opens the tab.
#[derive(Clone, Debug, PartialEq)]
pub enum PaletteEvent {
    OpenTab(TabSpec),
    OpenProjectQuery(String),
    /// Load SQL into the active query editor when conn matches.
    InjectSql {
        conn_id: ConnectionId,
        sql: String,
    },
    WorkspaceAction(WorkspacePaletteAction),
}

#[derive(Clone, Debug, PartialEq)]
pub enum WorkspacePaletteAction {
    NewLooseQuery,
    NewCollection,
    SelectNoEnvironment,
    OpenHome,
    OpenLogs,
    CheckForUpdates,
    OpenProject,
    OpenProjectInNewWindow,
    CloseProject,
}

/// A search result the palette can return: what it shows and what picking it does.
#[derive(Clone)]
pub struct PaletteResult {
    pub label: String,
    pub sublabel: String,
    pub conn_label: String,
    pub action: PaletteEvent,
}

/// One heading plus its rows, used as a `Command` group / `IndexPath` section.
#[derive(Clone)]
pub struct PaletteSection {
    pub heading: &'static str,
    pub items: Vec<PaletteResult>,
}
