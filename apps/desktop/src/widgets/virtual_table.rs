// VirtualTable: a DataTable<RowDelegate> for displaying typed cell rows.
// The DataTable widget in gpui-kit already virtualizes rows internally,
// so this is a thin wrapper / type alias for the RowDelegate-based table.

use std::collections::HashSet;
use std::rc::Rc;

use based_core::Value;
use gpui_kit::component::{
    StyleSized,
    menu::{ContextMenuExt, PopupMenu},
    table::{Column, ColumnSort, TableDelegate, TableState},
};
use gpui_kit::{prelude::*, *};

use crate::app::prefs;
use crate::widgets::cell_render::{grid_cell_kind, render_grid_cell};
use crate::widgets::column_header::{ColumnMeta, render_column_header, reorder_column_meta};
use crate::widgets::same_value::{self, SameValueMatch};

pub use crate::widgets::column_header::{align_meta_to_columns, meta_from_query_type};

/// One grid cell: the typed value and its display text, formatted once at load
/// so rendering a frame doesn't re-format every visible cell.
#[derive(Clone, Debug, PartialEq)]
pub struct Cell {
    pub value: Value,
    pub display: SharedString,
}

impl Cell {
    pub fn new(value: Value) -> Self {
        let display = value.display().into();
        Self { value, display }
    }

    /// A plain text cell (metadata grids, labels).
    pub fn text(s: impl Into<SharedString>) -> Self {
        let display: SharedString = s.into();
        Self {
            value: Value::text(display.as_ref()),
            display,
        }
    }

    pub fn is_null(&self) -> bool {
        self.value.is_null()
    }
}

impl Default for Cell {
    fn default() -> Self {
        Self::new(Value::Missing)
    }
}

impl From<Value> for Cell {
    fn from(value: Value) -> Self {
        Self::new(value)
    }
}

/// Typed values into grid rows.
pub fn cells(rows: Vec<Vec<Value>>) -> Vec<Vec<Cell>> {
    rows.into_iter()
        .map(|r| r.into_iter().map(Cell::new).collect())
        .collect()
}

/// Display text of every cell, e.g. for export and snapshots.
pub fn display_rows(rows: &[Vec<Cell>]) -> Vec<Vec<String>> {
    rows.iter()
        .map(|r| r.iter().map(|c| c.display.to_string()).collect())
        .collect()
}

/// Sortable, resizable column for query/browse grids.
pub fn data_column(key: impl Into<SharedString>, label: impl Into<SharedString>) -> Column {
    Column::new(key, label).sortable().resizable(true)
}

/// Builds the right-click menu for cell `(row, col)`.
pub type CellMenuFn =
    Rc<dyn Fn(PopupMenu, usize, usize, &mut Window, &mut Context<PopupMenu>) -> PopupMenu>;

/// Builds the right-click menu for the header of column `col`.
pub type HeaderMenuFn =
    Rc<dyn Fn(PopupMenu, usize, &mut Window, &mut Context<PopupMenu>) -> PopupMenu>;

/// Every column as delivered, so hidden ones can come back without a reload.
#[derive(Default)]
struct FullData {
    columns: Vec<Column>,
    meta: Vec<ColumnMeta>,
    rows: Vec<Vec<Cell>>,
}

/// Generic row data: column names + typed cells.
#[derive(Default)]
pub struct RowDelegate {
    pub columns: Vec<Column>,
    pub column_meta: Vec<ColumnMeta>,
    pub rows: Vec<Vec<Cell>>,
    pub sort_col: Option<usize>,
    pub sort_asc: bool,
    pub same_value_match: Option<SameValueMatch>,
    pub cell_focused: bool,
    pub cell_menu: Option<CellMenuFn>,
    pub header_menu: Option<HeaderMenuFn>,
    /// Keys of columns the user hid.
    hidden: HashSet<SharedString>,
    full: Option<FullData>,
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
        let header = render_column_header(col_ix, col.name.clone(), meta, window, cx);
        match self.header_menu.clone() {
            Some(f) => header
                .context_menu(move |menu, window, cx| f(menu, col_ix, window, cx))
                .into_any_element(),
            None => header.into_any_element(),
        }
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
        let highlight = same_value::cell_is_match(col_ix, &cell, self.same_value_match.as_ref());
        let is_null = cell.is_null();
        let display = cell.display;
        let meta = self.column_meta.get(col_ix).cloned().unwrap_or_default();
        let kind = grid_cell_kind(&cell.value, meta.data_type.as_deref(), display.as_ref());
        let cell_size = prefs::table_cell_size(cx);
        let cell = render_grid_cell(kind, display, is_null, row_ix, col_ix, window, cx);
        // Fill the kit cell (`relative` + `w(col_width)`). In-flow `bg` chips
        // to the glyphs and drops the kit left pad. Wash sits on this
        // unpadded fill; an inner `table_cell_size` keeps label inset. The
        // kit draws the blue ring after this node.
        let menu_fn = self.cell_menu.clone();
        let el = div()
            .id(("cell", row_ix << 16 | col_ix))
            .absolute()
            .inset_0()
            .when(highlight, |this| this.bg(same_value::wash_color()))
            .child(div().size_full().table_cell_size(cell_size).child(cell));
        match menu_fn {
            Some(f) => el
                .context_menu(move |menu, window, cx| f(menu, row_ix, col_ix, window, cx))
                .into_any_element(),
            None => el.into_any_element(),
        }
    }

    fn cell_text(&self, row_ix: usize, col_ix: usize, _: &App) -> String {
        self.rows[row_ix][col_ix].display.to_string()
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
        self.rows.sort_by(|a, b| {
            let ord = a[col_ix].value.cmp_for_sort(&b[col_ix].value);
            if asc { ord } else { ord.reverse() }
        });
        if let Some(needle) = self.same_value_match.take()
            && let Some(raw) = self
                .rows
                .get(needle.row_ix)
                .and_then(|row| row.get(needle.col_ix))
        {
            self.same_value_match = same_value::match_from_cell(needle.row_ix, needle.col_ix, raw);
        }
    }
}

/// Replace delegate data and rebuild gpui-kit column layout.
///
/// [`TableState::refresh`] must run after columns change; otherwise `col_groups` stays
/// empty from the initial delegate and body cells never render.
pub fn empty_column_meta(count: usize) -> Vec<ColumnMeta> {
    vec![ColumnMeta::default(); count]
}

pub fn replace_table_data(
    state: &mut TableState<RowDelegate>,
    columns: Vec<Column>,
    rows: Vec<Vec<Cell>>,
    column_meta: Vec<ColumnMeta>,
    cx: &mut Context<TableState<RowDelegate>>,
) {
    {
        let delegate = state.delegate_mut();
        let column_meta = if column_meta.len() == columns.len() {
            column_meta
        } else {
            empty_column_meta(columns.len())
        };
        delegate.full = Some(FullData {
            columns,
            meta: column_meta,
            rows,
        });
        apply_visibility(delegate);
    }
    refresh_same_value_highlight(state, None, cx);
    state.refresh(cx);
    cx.notify();
}

/// Rebuild the visible columns and rows from the full data, minus hidden columns.
fn apply_visibility(d: &mut RowDelegate) {
    let Some(full) = &d.full else {
        return;
    };
    let mut keep: Vec<usize> = (0..full.columns.len())
        .filter(|&i| !d.hidden.contains(&full.columns[i].key))
        .collect();
    if keep.is_empty() {
        keep = (0..full.columns.len()).collect();
    }
    d.columns = keep.iter().map(|&i| full.columns[i].clone()).collect();
    d.column_meta = keep
        .iter()
        .map(|&i| full.meta.get(i).cloned().unwrap_or_default())
        .collect();
    d.rows = full
        .rows
        .iter()
        .map(|r| {
            keep.iter()
                .map(|&i| r.get(i).cloned().unwrap_or_default())
                .collect()
        })
        .collect();
    d.sort_col = None;
}

/// `(key, name, visible)` for every column, in delivery order.
pub fn column_visibility(d: &RowDelegate) -> Vec<(SharedString, SharedString, bool)> {
    match &d.full {
        Some(full) => full
            .columns
            .iter()
            .map(|c| (c.key.clone(), c.name.clone(), !d.hidden.contains(&c.key)))
            .collect(),
        None => Vec::new(),
    }
}

/// Show or hide a column. The last visible column cannot be hidden.
pub fn toggle_column(
    state: &mut TableState<RowDelegate>,
    key: &SharedString,
    cx: &mut Context<TableState<RowDelegate>>,
) {
    {
        let d = state.delegate_mut();
        if d.hidden.remove(key) {
        } else if d.columns.len() > 1 {
            d.hidden.insert(key.clone());
        } else {
            return;
        }
        apply_visibility(d);
    }
    refresh_same_value_highlight(state, None, cx);
    state.refresh(cx);
    cx.notify();
}

/// Sort by `col_ix` from a menu, keeping the header arrows in step.
pub fn sort_column(
    state: &mut TableState<RowDelegate>,
    col_ix: usize,
    ascending: bool,
    window: &mut Window,
    cx: &mut Context<TableState<RowDelegate>>,
) {
    let sort = if ascending {
        ColumnSort::Ascending
    } else {
        ColumnSort::Descending
    };
    let d = state.delegate_mut();
    for (i, c) in d.columns.iter_mut().enumerate() {
        c.sort = Some(if i == col_ix {
            sort
        } else {
            ColumnSort::Default
        });
    }
    d.perform_sort(col_ix, sort, window, cx);
    state.refresh(cx);
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
            same_value::match_from_cell(row, col, raw)
        })
    } else {
        None
    };
    state.delegate_mut().same_value_match = needle;
}
