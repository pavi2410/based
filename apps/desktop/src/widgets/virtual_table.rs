// VirtualTable: a DataTable<RowDelegate> for displaying generic string-valued rows.
// The DataTable widget in gpui-component already virtualizes rows internally,
// so this is a thin wrapper / type alias for the RowDelegate-based table.

use gpui::{prelude::*, *};
use gpui_component::{
    ActiveTheme,
    table::{Column, ColumnSort, TableState},
};

use crate::app::prefs::{self, TableDensity};

pub const NULL_CELL_DISPLAY: &str = "NULL";

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
        let compact = prefs::table_density(cx) == TableDensity::Compact;
        let cell_pad = if compact {
            (px(8.0), px(2.0))
        } else {
            (px(12.0), px(6.0))
        };
        let mut el = div()
            .truncate()
            .px(cell_pad.0)
            .py(cell_pad.1)
            .font_family(cx.theme().mono_font_family.clone())
            .text_color(if is_null {
                cx.theme().muted_foreground
            } else {
                cx.theme().foreground
            });
        el = if compact { el.text_xs() } else { el.text_sm() };
        el.child(display)
    }

    fn cell_text(&self, row_ix: usize, col_ix: usize, _: &App) -> String {
        self.rows[row_ix][col_ix].to_string()
    }

    fn perform_sort(
        &mut self,
        col_ix: usize,
        sort: ColumnSort,
        _: &mut Window,
        _: &mut Context<TableState<Self>>,
    ) {
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
    cx.notify();
}
