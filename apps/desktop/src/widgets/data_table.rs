//! Shared `DataTable` styling for read-only string grids.

use gpui::Entity;
use gpui_component::table::{DataTable, TableDelegate, TableState};

/// Striped, borderless table — the default chrome for browse/query result grids.
pub fn read_only_striped<D>(table: &Entity<TableState<D>>) -> DataTable<D>
where
    D: TableDelegate,
{
    DataTable::new(table).stripe(true).bordered(false)
}
