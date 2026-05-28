# P0: Postgres-First Daily Driver Parity

Product and engineering specs for the P0 release. The source roadmap lives in the Cursor plan file; these documents are the implementation-facing artifacts.

| Document | Purpose |
|----------|---------|
| [PRD.md](./PRD.md) | Scope, principles, quality gates, success metrics |
| [track-a-connections.md](./track-a-connections.md) | Connection profiles, test/connect, session restore, error taxonomy |
| [track-b-explorer.md](./track-b-explorer.md) | Schema tree, search, refresh, DDL/data open |
| [track-c-sql-editor.md](./track-c-sql-editor.md) | Editor, execution modes, shortcuts, variables, cancellation |
| [track-d-results-data.md](./track-d-results-data.md) | Result grid, export, inline edits, save/discard |
| [track-e-history.md](./track-e-history.md) | Query history, favorites, rerun |
| [validation-checklist.md](./validation-checklist.md) | Workflow-based release validation |
| [release-readiness.md](./release-readiness.md) | Stable-default criteria and bug severity |
| [welcome-and-shell.md](./welcome-and-shell.md) | Welcome startup rules and main window layout |

## P0 product decisions (summary)

- **Postgres-first** depth; other engines are not P0 parity targets.
- **Global connection model** — no mandatory project/workspace abstraction for connections.
- **Welcome** (not a separate app shell) for first-run/empty state; returning users reopen last session when possible.
- **`.based` VCS** is not required for core P0 flows; global local stores are the source of truth for profiles/history in P0.

## Implementation map

### Library crates (no GPUI)

| Crate | Responsibility |
|-------|----------------|
| [`based-core`](../../crates/based-core) | `ConnectionId`, `TabId`, session persistence, connection error taxonomy |
| [`based-query`](../../crates/based-query) | History, saved queries, `$VAR` / `{{$…}}` resolution, SQL statement splitting |
| [`based-postgres`](../../crates/based-postgres) | `PostgresConfig`, SSL, `execute_sql`, DML helpers, EXPLAIN JSON parse |

### Desktop app (GPUI UI)

| Area | Primary modules |
|------|-----------------|
| Connections | `apps/desktop/src/connection/`, `postgres/wizard.rs` |
| Explorer | `workspace/connection_tree/`, `postgres/tree.rs` |
| SQL editor | `postgres/query_editor.rs`, `widgets/sql_editor.rs` |
| Data / results | `postgres/data_viewer.rs`, `widgets/data_table.rs` → `based-postgres` |
| History | `query_store/` (re-exports `based-query`), `workspace/chrome/panes/history_pane.rs` |
| Welcome | `workspace/welcome.rs` |
| Variables | `project/variables.rs` → `based-query` |
