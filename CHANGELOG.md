# Changelog

All notable changes to Based are documented here. Versions follow
[SemVer](https://semver.org). Entries target the desktop app
(`apps/desktop`) unless noted otherwise.

## 0.2.0 — Internal release

First internal cut of the DataGrip-class workspace. Focus areas:
make the three supported engines (PostgreSQL, SQLite, MongoDB)
productive in a single session, and get enough polish that other
people on the team can kick the tires.

### Added

- **Connection management**
  - New-connection wizard dialog (label, engine-specific fields, test
    button). Writes straight into `.based/config.toml`.
  - `test_connection` Rust command that probes a config without
    persisting it.
  - "Try sample project" on the welcome screen scaffolds a SQLite-
    backed blog dataset (users / posts / comments) so new users can
    explore immediately.

- **Workspace**
  - Tabbed workspace with drag-to-detach (tabs pop out into their own
    OS windows).
  - Command palette (`⌘K` / `Ctrl+K`) with quick actions (new query,
    close tab, open settings, toggle theme).
  - Global keyboard shortcuts: `⌘T` new query, `⌘W` close tab,
    `⌘Enter` run, `⌘S` save.
  - SQL autocomplete in the query editor uses cached schema metadata
    (no extra IPC roundtrip).
  - Beginner / Pro mode toggle; Beginner hides advanced actions
    (EXPLAIN, Mongo aggregate builder).

- **Queries**
  - Templated `:param` substitution for saved queries with a UI panel
    to edit parameter values.
  - Per-execution timeout selector.
  - Cancel-running-query support, backed by a cooperative
    `QueryRegistry` on the Rust side.
  - Persistent per-connection query history (localStorage, bounded to
    200 entries) with click-to-prefill.
  - EXPLAIN / EXPLAIN ANALYZE menu for SQL engines.
  - MongoDB aggregate stage builder (insert canned pipeline stages).

- **Data grid**
  - Virtualized rows + columns.
  - Cell detail side panel for JSON / long text.
  - Export to CSV / JSON.
  - Multi-column sort and filter AST with bind parameters.
  - Primary-key-aware inline row edit + insert + delete with undo
    and readonly enforcement.
  - Pop-out result / table windows.

- **Chrome**
  - Status bar at the bottom of the workspace (engine badge,
    connection time, in-flight query count, UI-mode indicator).
  - Settings window (theme, UI mode).
  - Error boundaries around the router, detached windows, and each
    tab surface.

### Changed

- Schema cache sits in front of every engine call; redundant
  `list_tables` / `describe_table` lookups collapse into cache hits.
- `project_db_commands.rs` split along engine lines via
  `EngineCapability`. Frontend `EngineAdapter` mirrors the split.
- Filter generation uses an AST with bound parameters and a shared
  `value_to_json` + `quote_ident` helper; previously string-built.
- React Query keys are typed end-to-end.
- Vite manualChunks config splits CodeMirror, Radix, and
  react-table/virtual into their own bundles. Main entry dropped from
  ~840KB to ~630KB; the connection route chunk from ~730KB to ~85KB.

### Infra

- Vitest harness with happy-dom; 16 frontend unit tests cover the
  tabs, query-history, and running-query stores.
- Tempfile-backed Rust tests for `create_sample_project`.
- CI now runs `vitest run` alongside tsc/build/biome/fmt/clippy/test.

### Known gaps (targeted for 0.3.0)

- No auto-updater / code signing / notarization. Distribution is
  still "build locally" until a real public release.
- Secret values shown as plaintext in the wizard; env-var / file
  references still require hand-editing `config.toml`.
- Settings window exposes only theme + UI mode. More prefs land once
  we know which defaults actually need per-device overrides.
- Bundle is still client-rendered with no preloading between chunks;
  a preload + prefetch pass is next.
