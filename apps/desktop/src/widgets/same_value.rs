//! Focus-driven same-value highlight, scoped to one column.

use gpui_kit::{Hsla, SharedString, rgb};

use crate::widgets::virtual_table::Cell;

/// Warm gray `#57524f` at ~1/3 so the row / zebra color still shows through.
pub fn wash_color() -> Hsla {
    Hsla::from(rgb(0x57524f)).opacity(1.0 / 3.0)
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SameValueMatch {
    pub row_ix: usize,
    pub col_ix: usize,
    pub value: SharedString,
}

pub fn match_from_cell(row_ix: usize, col_ix: usize, cell: &Cell) -> Option<SameValueMatch> {
    if cell.is_null() {
        return None;
    }
    Some(SameValueMatch {
        row_ix,
        col_ix,
        value: cell.display.clone(),
    })
}

pub fn cell_is_match(col_ix: usize, cell: &Cell, needle: Option<&SameValueMatch>) -> bool {
    let Some(needle) = needle else {
        return false;
    };
    if col_ix != needle.col_ix || cell.is_null() {
        return false;
    }
    cell.display == needle.value
}

pub fn remap_column(col_ix: usize, from: usize, to: usize) -> usize {
    let insert_at = if to > from { to - 1 } else { to };
    match col_ix {
        c if c == from => insert_at,
        c if from < to && c > from && c < to => c - 1,
        c if from > to && c >= to && c < from => c + 1,
        c => c,
    }
}
