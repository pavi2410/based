use crate::connection::ConnectionId;
use crate::workspace::tab_spec::TabSpec;

/// Emitted when the user picks a palette row — workspace opens the tab.
#[derive(Clone, Debug)]
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

#[derive(Clone, Debug)]
pub enum WorkspacePaletteAction {
    NewLooseQuery,
    NewCollection,
    SelectNoEnvironment,
    OpenHome,
    OpenOnboarding,
    CheckForUpdates,
    OpenProject,
    OpenProjectInNewWindow,
}

/// A search result the palette can return.
#[derive(Clone)]
#[allow(dead_code)]
pub struct PaletteResult {
    pub kind: ResultKind,
    pub label: String,
    pub sublabel: String,
    pub conn_label: String,
    pub spec: TabSpec,
    pub project_query_path: Option<String>,
    pub command_action: Option<WorkspacePaletteAction>,
}

#[derive(Clone, PartialEq, Eq)]
pub enum ResultKind {
    SchemaObject,
    SavedQuery,
    History,
    Command,
}
