// VirtualTable: a DataTable<RowDelegate> for displaying generic string-valued rows.
// The DataTable widget in gpui-component already virtualizes rows internally,
// so this is a thin wrapper / type alias for the RowDelegate-based table.

use gpui::{prelude::*, *};
use gpui_component::{
    ActiveTheme, StyleSized,
    table::{Column, ColumnSort, TableState},
};

use crate::app::prefs;

pub const NULL_CELL_DISPLAY: &str = "NULL";

/// Sortable, resizable column for query/browse grids.
pub fn data_column(key: impl Into<SharedString>, label: impl Into<SharedString>) -> Column {
    Column::new(key, label).sortable().resizable(true)
}

/// Generic row data: column names + string-valued cells.
#[derive(Default)]
pub struct RowDelegate {
    pub columns: Vec<Column>,
    pub rows: Vec<Vec<SharedString>>,
    pub sort_col: Option<usize>,
    pub sort_asc: bool,
}

impl gpui_component::table::TableDelegate for RowDelegate {
    fn columns_count(&self, _: &App) -> usize {
        self.columns.len()
    }

    fn rows_count(&self, _: &App) -> usize {
        self.rows.len()
    }

    fn column(&self, col_ix: usize, _: &App) -> Column {
        self.columns[col_ix].clone()
    }

    fn render_td(
        &mut self,
        row_ix: usize,
        col_ix: usize,
        _: &mut Window,
        cx: &mut Context<TableState<Self>>,
    ) -> impl IntoElement {
        let cell = self
            .rows
            .get(row_ix)
            .and_then(|row| row.get(col_ix))
            .cloned()
            .unwrap_or_default();
        let is_null = cell.is_empty() || cell.as_ref() == NULL_CELL_DISPLAY;
        let display: SharedString = if cell.is_empty() {
            NULL_CELL_DISPLAY.into()
        } else {
            cell
        };
        div()
            .truncate()
            .table_cell_size(prefs::table_cell_size(cx))
            .font_family(crate::app::prefs::code_font_family(cx))
            .text_color(if is_null {
                cx.theme().muted_foreground
            } else {
                cx.theme().foreground
            })
            .child(display)
    }

    fn cell_text(&self, row_ix: usize, col_ix: usize, _: &App) -> String {
        self.rows[row_ix][col_ix].to_string()
    }

    fn move_column(
        &mut self,
        col_ix: usize,
        to_ix: usize,
        _: &mut Window,
        _: &mut Context<TableState<Self>>,
    ) {
        if col_ix >= self.columns.len() || to_ix > self.columns.len() {
            return;
        }
        let col = self.columns.remove(col_ix);
        let insert_at = if to_ix > col_ix { to_ix - 1 } else { to_ix };
        self.columns.insert(insert_at, col);

        for row in &mut self.rows {
            if col_ix >= row.len() {
                continue;
            }
            let cell = row.remove(col_ix);
            let insert_at = insert_at.min(row.len());
            row.insert(insert_at, cell);
        }

        if let Some(sort_col) = self.sort_col {
            self.sort_col = Some(match sort_col {
                c if c == col_ix => insert_at,
                c if col_ix < to_ix && c > col_ix && c < to_ix => c - 1,
                c if col_ix > to_ix && c >= to_ix && c < col_ix => c + 1,
                c => c,
            });
        }
    }

    fn perform_sort(
        &mut self,
        col_ix: usize,
        sort: ColumnSort,
        _: &mut Window,
        cx: &mut Context<TableState<Self>>,
    ) {
        if !prefs::table_prefs(cx).sortable {
            return;
        }
        self.sort_col = Some(col_ix);
        self.sort_asc = matches!(sort, ColumnSort::Ascending);
        if self.sort_asc {
            self.rows.sort_by(|a, b| a[col_ix].cmp(&b[col_ix]));
        } else {
            self.rows.sort_by(|a, b| b[col_ix].cmp(&a[col_ix]));
        }
    }
}

pub type VirtualTable = Entity<TableState<RowDelegate>>;

/// Replace delegate data and rebuild gpui-component column layout.
///
/// [`TableState::refresh`] must run after columns change; otherwise `col_groups` stays
/// empty from the initial delegate and body cells never render.
pub fn replace_table_data(
    state: &mut TableState<RowDelegate>,
    columns: Vec<Column>,
    rows: Vec<Vec<SharedString>>,
    cx: &mut Context<TableState<RowDelegate>>,
) {
    let delegate = state.delegate_mut();
    delegate.columns = columns;
    delegate.rows = rows;
    delegate.sort_col = None;
    state.refresh(cx);
    cx.notify();
}

/// Replace row data when columns are unchanged.
pub fn replace_table_rows(
    state: &mut TableState<RowDelegate>,
    rows: Vec<Vec<SharedString>>,
    cx: &mut Context<TableState<RowDelegate>>,
) {
    state.delegate_mut().rows = rows;
    state.delegate_mut().sort_col = None;
    cx.notify();
}
