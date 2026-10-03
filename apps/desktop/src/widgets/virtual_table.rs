// VirtualTable: a DataTable<RowDelegate> for displaying generic string-valued rows.
// The DataTable widget in gpui-kit already virtualizes rows internally,
// so this is a thin wrapper / type alias for the RowDelegate-based table.

use gpui_kit::component::table::{Column, ColumnSort, TableDelegate, TableState};
use gpui_kit::{prelude::*, *};

use crate::app::prefs;
use crate::widgets::cell_render::{
    cell_value_kind, column_value_kind, compare_cells, render_grid_cell,
};
use crate::widgets::column_header::{GridColumnMeta, render_column_header, reorder_column_meta};
use crate::widgets::same_value::{self, SameValueMatch};

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
    pub same_value_match: Option<SameValueMatch>,
    pub cell_focused: bool,
}

impl TableDelegate for RowDelegate {
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
        let highlight = same_value::cell_is_match(
            row_ix,
            col_ix,
            cell.as_ref(),
            self.same_value_match.as_ref(),
        );
        let is_null = cell.is_empty() || cell.as_ref() == NULL_CELL_DISPLAY;
        let display: SharedString = if cell.is_empty() {
            NULL_CELL_DISPLAY.into()
        } else {
            cell
        };
        let meta = self.column_meta.get(col_ix).cloned().unwrap_or_default();
        let kind = cell_value_kind(meta.data_type.as_deref(), display.as_ref());
        let cell = render_grid_cell(kind, display, is_null, row_ix, col_ix, window, cx);
        // In-flow fill, same as column headers (`flex_1` + `min_w_0` +
        // `size_full`). `cell_chrome` shrinks to the glyphs; gpui-kit's
        // `absolute().inset_0()` selection overlay then sizes to that
        // content, not the cell's `.w(col_width)`. A full-size td box
        // makes both the wash and the blue ring cover the cell.
        div()
            .flex_1()
            .min_w_0()
            .size_full()
            .when(highlight, |this| this.bg(same_value::wash_color()))
            .child(cell)
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
        if let Some(needle) = &mut self.same_value_match {
            needle.col_ix = same_value::remap_column(needle.col_ix, col_ix, to_ix);
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
        if let Some(needle) = self.same_value_match.take()
            && let Some(raw) = self
                .rows
                .get(needle.row_ix)
                .and_then(|row| row.get(needle.col_ix))
        {
            self.same_value_match =
                same_value::match_from_cell(needle.row_ix, needle.col_ix, raw.as_ref());
        }
    }
}

pub type VirtualTable = Entity<TableState<RowDelegate>>;

/// Replace delegate data and rebuild gpui-kit column layout.
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
    {
        let delegate = state.delegate_mut();
        delegate.columns = columns;
        delegate.column_meta = if column_meta.len() == delegate.columns.len() {
            column_meta
        } else {
            empty_column_meta(delegate.columns.len())
        };
        delegate.rows = rows;
        delegate.sort_col = None;
    }
    refresh_same_value_highlight(state, None, cx);
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
    refresh_same_value_highlight(state, None, cx);
    cx.notify();
}

pub fn refresh_same_value_highlight(
    state: &mut TableState<RowDelegate>,
    cell: Option<(usize, usize)>,
    cx: &App,
) {
    let prefs = prefs::table_prefs(cx);
    let enabled = prefs.highlight_same_value && prefs.cell_selectable;
    let focused = state.delegate().cell_focused;
    let cell = cell.or_else(|| state.selected_cell());
    let needle = if enabled && focused {
        cell.and_then(|(row, col)| {
            let raw = state.delegate().rows.get(row)?.get(col)?;
            same_value::match_from_cell(row, col, raw.as_ref())
        })
    } else {
        None
    };
    state.delegate_mut().same_value_match = needle;
}
