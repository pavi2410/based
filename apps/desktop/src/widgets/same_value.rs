//! Focus-driven same-value highlight, scoped to one column.

use gpui_kit::SharedString;

use crate::widgets::cell_render::is_null_cell;

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SameValueMatch {
    pub row_ix: usize,
    pub col_ix: usize,
    pub value: SharedString,
}

pub fn match_from_cell(row_ix: usize, col_ix: usize, raw: &str) -> Option<SameValueMatch> {
    if is_null_cell(raw) {
        return None;
    }
    Some(SameValueMatch {
        row_ix,
        col_ix,
        value: raw.to_string().into(),
    })
}

pub fn cell_is_match(
    row_ix: usize,
    col_ix: usize,
    raw: &str,
    needle: Option<&SameValueMatch>,
) -> bool {
    let Some(needle) = needle else {
        return false;
    };
    if col_ix != needle.col_ix || row_ix == needle.row_ix || is_null_cell(raw) {
        return false;
    }
    raw == needle.value.as_ref()
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
