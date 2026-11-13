//! Read-only view of a query editor's results, for tests and inspection.

use gpui_kit::App;
use std::ops::Range;

use super::query_editor::{QueryEditorPanel, QueryStatus};
use crate::widgets::virtual_table::display_rows;

/// What the results grid and status line currently show.
#[derive(Debug, Default, PartialEq, Eq)]
pub struct ResultSnapshot {
    pub columns: Vec<String>,
    pub rows: Vec<Vec<String>>,
    pub running: bool,
    pub error: Option<String>,
    /// The editor text the error points at.
    pub error_span: Option<Range<usize>>,
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
            rows: display_rows(&grid.rows),
            running: self.status.is_running(),
            result_count: self.result_count(),
            active_result: self.active_result(),
            error: match &self.status {
                QueryStatus::Error { message, .. } => Some(message.clone()),
                _ => None,
            },
            error_span: self.status.error_span(),
        }
    }
}
