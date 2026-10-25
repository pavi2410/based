//! Read/drive API over a data viewer page, for tests and inspection.

use gpui_kit::{App, Context};

use super::data_viewer::DataViewerPanel;

/// What the grid and pager currently show.
#[derive(Debug, Default, PartialEq, Eq)]
pub struct PageSnapshot {
    pub columns: Vec<String>,
    pub rows: Vec<Vec<String>>,
    pub total: u64,
    pub offset: u64,
    pub page_size: u64,
    pub loading: bool,
}

impl DataViewerPanel {
    pub fn page_snapshot(&self, cx: &App) -> PageSnapshot {
        let grid = self.table.read(cx).delegate();
        PageSnapshot {
            columns: grid.columns.iter().map(|c| c.key.to_string()).collect(),
            rows: grid
                .rows
                .iter()
                .map(|row| row.iter().map(|c| c.to_string()).collect())
                .collect(),
            total: self.total_rows,
            offset: self.offset,
            page_size: self.page_size,
            loading: self.loading,
        }
    }

    /// Jump to a 1-based page, as the pager does.
    pub fn show_page(&mut self, page: usize, cx: &mut Context<Self>) {
        self.go_to_page(page, cx);
    }
}
