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
    /// How many result sets the last run produced.
    pub result_count: usize,
    /// Which of them the grid shows.
    pub active_result: usize,
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
            result_count: self.results.len(),
            active_result: self.active_result,
            error: match &self.status {
                QueryStatus::Error(e) => Some(e.clone()),
                _ => None,
            },
        }
    }
}
