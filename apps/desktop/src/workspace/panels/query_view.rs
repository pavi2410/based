//! Read-only view of a query editor's results, for tests and inspection.

use gpui_kit::App;

use super::query_editor::{QueryEditorPanel, QueryStatus};

/// What the results grid and status line currently show.
#[derive(Debug, Default, PartialEq, Eq)]
pub struct ResultSnapshot {
    pub columns: Vec<String>,
    pub rows: Vec<Vec<String>>,
    pub running: bool,
    pub error: Option<String>,
}

impl QueryEditorPanel {
    pub fn result_snapshot(&self, cx: &App) -> ResultSnapshot {
        let grid = self.result.read(cx).delegate();
        ResultSnapshot {
            columns: grid.columns.iter().map(|c| c.key.to_string()).collect(),
            rows: grid
                .rows
                .iter()
                .map(|row| row.iter().map(|c| c.to_string()).collect())
                .collect(),
            running: matches!(self.status, QueryStatus::Running),
            error: match &self.status {
                QueryStatus::Error(e) => Some(e.clone()),
                _ => None,
            },
        }
    }
}
