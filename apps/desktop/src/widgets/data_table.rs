//! Shared `DataTable` factory and styling for read-only string grids.

use gpui_kit::component::{
    Sizable,
    table::{DataTable, TableEvent, TableState},
};
use gpui_kit::{App, Context, Entity, Window};

use crate::app::prefs;
use crate::widgets::virtual_table::{RowDelegate, refresh_same_value_highlight};

/// Create a [`TableState`] for string grids with interaction prefs applied.
pub fn configure_row_table(
    delegate: RowDelegate,
    window: &mut Window,
    cx: &mut Context<TableState<RowDelegate>>,
) -> TableState<RowDelegate> {
    let prefs = prefs::table_prefs(cx);
    let state = TableState::new(delegate, window, cx)
        .loop_selection(prefs.loop_selection)
        .col_resizable(prefs.col_resizable)
        .col_movable(prefs.col_movable)
        .sortable(prefs.sortable)
        .row_selectable(prefs.row_selectable)
        .cell_selectable(prefs.cell_selectable);

    let entity = cx.entity();
    cx.subscribe(&entity, |this, _, event, cx| {
        apply_same_value_event(this, event, cx);
    })
    .detach();
    state
}

fn apply_same_value_event(state: &mut TableState<RowDelegate>, event: &TableEvent, cx: &App) {
    match event {
        TableEvent::SelectCell(row, col) => {
            state.delegate_mut().cell_focused = true;
            refresh_same_value_highlight(state, Some((*row, *col)), cx);
        }
        TableEvent::SelectRow(_) | TableEvent::SelectColumn(_) | TableEvent::ClearSelection => {
            let delegate = state.delegate_mut();
            delegate.cell_focused = false;
            delegate.same_value_match = None;
        }
        _ => {}
    }
}

/// Sync live table interaction prefs from global settings.
pub fn sync_table_prefs(state: &mut TableState<RowDelegate>, cx: &App) {
    let prefs = prefs::table_prefs(cx);
    state.loop_selection = prefs.loop_selection;
    state.col_resizable = prefs.col_resizable;
    state.col_movable = prefs.col_movable;
    state.sortable = prefs.sortable;
    state.row_selectable = prefs.row_selectable;
    state.cell_selectable = prefs.cell_selectable;
    refresh_same_value_highlight(state, None, cx);
}

/// Render a string grid with stripe, border, and density from prefs.
pub fn render_row_table(
    table: &Entity<TableState<RowDelegate>>,
    cx: &mut App,
) -> DataTable<RowDelegate> {
    table.update(cx, |state, cx| sync_table_prefs(state, cx));
    let prefs = prefs::table_prefs(cx);
    DataTable::new(table)
        .stripe(prefs.stripe)
        .bordered(true)
        .with_size(prefs::table_cell_size(cx))
}
