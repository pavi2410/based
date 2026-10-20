# Changelog

## v2026.10.0

### Added
- Redis engine: key browser, key viewer and console.
- SQL editor: formatting, autocomplete for Postgres and Redis, context-aware column completion (FROM/JOIN scope and aliases), identifier quoting, and syntax-error / schema-lint diagnostics.
- Tabs: kind icons, hover info, right-click context menu, per-tab close button (max width 120px).
- Database switcher above the catalog for Postgres and Redis.
- Same-value column highlight in result grids (Settings toggle).
- Datetime hover card on result-grid cells.
- New-connection button in the icon rail.
- Rotating log file and Help → Open Logs.
- Icons on filter, export, refresh and run buttons.

### Changed
- Desktop app now uses gpui-kit (0.8) as its only GPUI dependency; dock, command palette and overlays migrated.
- Table viewer pagination grouped into the footer.
- Toggle Sidebar hides only the catalog pane; Queries pane collapses when empty.
- Status dot shows only when connected; history status removed from the status bar.
- MongoDB driver upgraded 2.8 → 3.9, with public-DNS fallback for `mongodb+srv`.
- Removed the first-run setup window.
- Shared query editor, table inspector and data viewer panels across SQLite and Postgres; engine logic moved into engine crates.
- Session saves and query-lane store writes run off the UI thread.
- Release profile tuned; dead code removed.

### Fixed
- Postgres query cells decoded by type (blank cells).
- Session restore: no longer replaced by dashboard plus blank query; tabs kept until connection is up; Dashboard tab serialization.
- Closing Redis console and key view tabs.
- Duplicate connection names are rejected on save.
- Postgres schemas always nest in the catalog.
- Per-frame quit dialog log spam.
