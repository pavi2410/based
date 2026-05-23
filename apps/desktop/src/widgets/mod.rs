// widgets/ — engine-agnostic UI primitives.
// None of these know what a database is; they operate on plain data types.
// Implemented progressively across Phases 1–3.

pub mod cell_detail;
pub mod data_table;
pub mod filter_bar;
pub mod list_row;
pub mod pagination;
pub mod sql_editor;
pub mod status_glyph;
pub mod tab_chip;
pub mod ui;
pub mod virtual_table;
