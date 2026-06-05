// VirtualTable: a DataTable<RowDelegate> for displaying generic string-valued rows.
// The DataTable widget in gpui-component already virtualizes rows internally,
// so this is a thin wrapper / type alias for the RowDelegate-based table.

use gpui::{prelude::*, *};
use gpui_component::table::{Column, ColumnSort, TableState};

use crate::app::prefs;
use crate::widgets::cell_render::{column_value_kind, compare_cells, render_grid_cell};
use crate::widgets::column_header::{GridColumnMeta, render_column_header, reorder_column_meta};

pub use crate::widgets::column_header::{align_meta_to_columns, meta_from_query_type};

pub const NULL_CELL_DISPLAY: &str = "NULL";

/// Sortable, resizable column for query/browse grids.
pub fn data_column(key: impl Into<SharedString>, label: impl Into<SharedString>) -> Column {
    Column::new(key, label).sortable().resizable(true)
}

/// Generic row data: column names + string-valued cells.
#[derive(Default)]
pub struct RowDelegate {
    pub columns: Vec<Column>,
    pub column_meta: Vec<GridColumnMeta>,
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

    fn render_th(
        &mut self,
        col_ix: usize,
        window: &mut Window,
        cx: &mut Context<TableState<Self>>,
    ) -> impl IntoElement {
        let col = self.column(col_ix, cx);
        let meta = self.column_meta.get(col_ix).cloned().unwrap_or_default();
        render_column_header(col_ix, col.name.clone(), meta, window, cx)
    }

    fn render_td(
        &mut self,
        row_ix: usize,
        col_ix: usize,
        window: &mut Window,
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
        let meta = self.column_meta.get(col_ix).cloned().unwrap_or_default();
        let kind = column_value_kind(meta.data_type.as_deref());
        render_grid_cell(kind, display, is_null, row_ix, col_ix, window, cx)
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
        reorder_column_meta(&mut self.column_meta, col_ix, to_ix);

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
        let asc = self.sort_asc;
        let meta = self.column_meta.get(col_ix).cloned().unwrap_or_default();
        let kind = column_value_kind(meta.data_type.as_deref());
        self.rows.sort_by(|a, b| {
            let ord = compare_cells(kind, a[col_ix].as_ref(), b[col_ix].as_ref());
            if asc { ord } else { ord.reverse() }
        });
    }
}

pub type VirtualTable = Entity<TableState<RowDelegate>>;

/// Replace delegate data and rebuild gpui-component column layout.
///
/// [`TableState::refresh`] must run after columns change; otherwise `col_groups` stays
/// empty from the initial delegate and body cells never render.
pub fn empty_column_meta(count: usize) -> Vec<GridColumnMeta> {
    vec![GridColumnMeta::default(); count]
}

pub fn replace_table_data(
    state: &mut TableState<RowDelegate>,
    columns: Vec<Column>,
    rows: Vec<Vec<SharedString>>,
    column_meta: Vec<GridColumnMeta>,
    cx: &mut Context<TableState<RowDelegate>>,
) {
    let delegate = state.delegate_mut();
    delegate.columns = columns;
    delegate.column_meta = if column_meta.len() == delegate.columns.len() {
        column_meta
    } else {
        empty_column_meta(delegate.columns.len())
    };
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
