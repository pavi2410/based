// widgets/ — engine-agnostic UI primitives.
// None of these know what a database is; they operate on plain data types.

pub mod cell_detail;
pub mod command_shell;
pub mod data_table;
pub mod description_list;
pub mod empty_state;
pub mod engine;
pub mod export;
pub mod export_popover;
pub mod filter_bar;
pub mod kbd;
pub mod layout;
pub mod list_row;
pub mod metadata_pill;
pub mod pagination;
pub mod panel;
pub mod query_panel_extras;
pub mod query_status;
pub mod result_tabs;
pub mod row_cell;
pub mod section_eyebrow;
pub mod sql_editor;
pub mod status_glyph;
pub mod status_item;
pub mod tab_chip;
pub mod virtual_table;

// Flat re-exports — callers use `crate::widgets::X` instead of `crate::widgets::submod::X`.
pub use command_shell::*;
pub use description_list::*;
pub use engine::*;
pub use kbd::*;
pub use layout::*;
pub use metadata_pill::*;
pub use panel::*;
