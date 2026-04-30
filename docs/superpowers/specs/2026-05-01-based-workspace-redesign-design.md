# Based — Workspace Redesign & Feature Design Spec

**Date:** 2026-05-01  
**Status:** Approved  
**Scope:** Workspace-first redesign + v1 feature set across PostgreSQL, SQLite, MongoDB

---

## 1. Product Context

**Based** is a native desktop database client (Rust + GPUI) targeting solo developers and product team developers who use PostgreSQL, SQLite, and MongoDB in production — all three engines co-equally.

**Killer workflows (priority order):**
1. Query experience — write SQL/pipelines fast, see results instantly, save and revisit, explain inline
2. Multi-connection switching — juggle local SQLite, staging Postgres, prod Mongo simultaneously without losing context
3. Workspace model — `.based/` folder pre-configured with all connections, drop straight into data exploration

**Out of scope for v1:** migrations, schema management, live monitoring, DDL editing, bulk DML.

**Design language:** Modern native productivity minimalism. Graphite dark surfaces, hairline borders, devicon engine logos, engine-colored accents (Postgres blue `#336791`, SQLite teal `#57aaa0`, MongoDB green `#13aa52`), monospace where it matters, command palette as the fast path. Inspirations: DataGrip, Beekeeper Studio, TablePlus.

---

## 2. Implementation Strategy

**Approach: Workspace-first.**

The current `workspace/mod.rs` (1,490 lines) owns six different concerns. The refactor extracts each into its own entity with a single responsibility. Every subsequent feature then has a clean home and can be built in isolation.

Sequence:
1. Workspace shell refactor (ConnectionTree + TabManager split)
2. Command palette
3. QueryStore (history + saved queries)
4. Engine panel completions + stub fixes
5. Shared widget completions (filter bar, cell detail, code editor)

---

## 3. Architecture

### Component layers

```
OS / GPUI layer
  gpui_platform          — window, input, render loop          [keep]
  gpui_tokio             — Tokio ↔ GPUI async bridge (db.rs)  [keep]
  gpui-component         — DockArea, button, virtual table     [keep]

App shell
  ConnectionRegistry     — all connections, state machine      [keep]
  WorkspaceState         — tab layout + session persistence     [refactor]
  CommandPalette         — ⌘K global search                   [new]
  QueryStore             — history + saved queries             [new]

Workspace
  Workspace (root)       — orchestrates sidebar + tabs + bars  [refactor]
  ConnectionTree         — nested sidebar tree                 [new]
  TabManager             — heterogeneous tabs, lifecycle       [refactor]

Panels (per engine)
  DataViewer             — paginated grid, filter, sort        [refactor]
  QueryEditor            — SQL/pipeline editor, results        [refactor]
  SchemaInspector        — columns, indexes, constraints       [fix stubs]
  ExplainViewer          — EXPLAIN node tree                   [refactor]
  ConnectionDashboard    — landing panel on connect            [fix stubs]
  DocumentEditor         — MongoDB insert/edit (JSON)          [fix stub]

Engines (unchanged)
  PgConnection           — sqlx::PgPool                       [keep]
  SqliteConnection       — sqlx::SqlitePool                   [keep]
  MongoConnection        — mongodb::Client                    [keep]
```

### Key architectural shift

`Workspace` becomes a pure orchestrator. `ConnectionTree` owns sidebar state (expand/collapse, cached schema, selected object). `TabManager` owns the tab vec and active panel. `CommandPalette` and `QueryStore` are global entities that subscribe to `ConnectionRegistry` and each other directly — no prop-drilling.

### Key interaction flow

```
User clicks object in ConnectionTree
  → TabManager.open_or_focus(TabSpec::DataViewer(conn_id, object))
  → DataViewer spawns async fetch via db.rs / Tokio
  → Grid renders with engine color in tab indicator + breadcrumb
  → WorkspaceState persists open tabs to .based/local/workspace.json
```

---

## 4. Connection Tree

The sidebar is **navigation only**. Its job: find things, open them as tabs. Nothing else.

### Structure

```
WORKSPACE                        ← header
▾ [pg icon]  prod-pg       ●    ← connection row, connected
    TABLES              12
      users                      ← object row (active: engine-colored left border)
      orders
    VIEWS               3
      user_summary
▾ [sqlite]   local-sqlite  ●
    TABLES              4
      notes
▸ [mongo]    analytics-mongo ●  ← collapsed
[mongo]      staging-pg  ⟳      ← connecting (amber pulsing dot)
＋ Add connection
```

### Schema groups per engine

| Engine | Groups |
|--------|--------|
| PostgreSQL | Tables, Views, Materialized Views (Functions — v2) |
| SQLite | Tables, Views, Triggers (FTS5 tables badged within Tables) |
| MongoDB | Collections, Views, Indexes (nested under collection) |

### Interaction rules

- **Single click object** → open DataViewer / DocumentViewer tab (or focus if already open)
- **Hover object** → inline `View` button appears for all engines. No `Query` button on object rows — queries are connection-level for SQL engines; for MongoDB, "Open in Pipeline Builder" lives in the right-click menu only.
- **Right-click object** → context menu (see below)
- **Hover connection row** → `New Query` button appears (opens QueryEditor for SQL engines; no equivalent for MongoDB — pipelines are collection-scoped and open via right-click)
- **Click collapsed connection** → expand; if disconnected, initiate connect
- **Collapse connection** → hides tree, keeps tabs alive, keeps connection alive

### Connection states

| State | Dot | Click action |
|-------|-----|--------------|
| Connected | green solid | Collapse / expand |
| Connecting | amber pulsing | Cancel |
| Disconnected | grey | Connect |
| Failed | red | Retry / Edit config |

### Schema loading

Lazy: schema groups load on first expand, cached after. Manual refresh button on group header. Connect-time is fast — no schema query blocks it.

### Right-click context menu (table/view/collection)

```
Open as
  View Data              ↵   (default action)
  Inspect Structure
  Open in Query Editor       (MongoDB: Open in Pipeline Builder)

Copy
  Copy Table Name
  Copy SELECT Statement

Refresh Schema
```

No destructive actions. DROP/TRUNCATE go through the Query Editor intentionally.

---

## 5. Tab System

Tabs are the primary work surface. Each tab is a self-contained panel — any engine, any content type — open simultaneously.

### Tab anatomy

Each tab shows:
- **3px left-side indicator bar** in engine color (Postgres blue, SQLite green, MongoDB green)
- **Kind label** in small dim text (e.g. `data viewer`, `query`, `pipeline`)
- **Object/title label** (e.g. `users`, `revenue.sql`, `untitled ●`)
- **Amber dot** on unsaved query tabs

### Tab types

| Type | Engines | Opens from |
|------|---------|------------|
| Connection Dashboard | all | On connect (auto) |
| Data Viewer | all | Click object / context menu "View Data" |
| SQL Query Editor | pg, sqlite | Connection row "New Query" / context menu |
| Pipeline Builder | mongo | Collection context menu / palette |
| Explain Viewer | pg, sqlite | ⌥↵ in Query Editor |
| Schema Inspector | all | Context menu "Inspect Structure" |

### Tab lifecycle rules

- Clicking an already-open object **focuses** the existing tab — never opens a duplicate DataViewer
- New Query always opens a **fresh** untitled QueryEditor (queries are not objects, duplicates are fine)
- Closing a tab with unsaved query content shows **inline confirmation** (not a modal)
- When a connection disconnects, its tabs **go grey** — not closed, data preserved
- Session restore: tabs saved in `.based/local/workspace.json` reopen on launch, data loads lazily
- `⌘W` close active tab · `⌘⇧]` / `⌘⇧[` next/previous tab

### TabManager implementation note

`TabSpec` is an enum: `DataViewer(conn_id, object)`, `QueryEditor(conn_id)`, `Pipeline(conn_id, collection)`, `Explain(conn_id, plan)`, `Inspector(conn_id, object)`, `Dashboard(conn_id)`. Active panel entity is kept alive on tab switch — scroll position and query text survive.

---

## 6. Command Palette (⌘K)

Global search overlay. Searches across all connected engines simultaneously.

### Search scope (in result priority order)

1. Tables, views, collections — from all connected engines
2. Saved queries — from QueryStore
3. Recent history — last 20 entries from QueryStore

### Behaviours

- **↵** on a table/collection → open DataViewer tab
- **⌘↵** on a table → open QueryEditor pre-filled with `SELECT * FROM table LIMIT 100`
- **⌘↵** on a saved query → open in QueryEditor for editing
- **↵** on a history entry → restore into active QueryEditor
- Engine chip on each result disambiguates same-named tables across connections
- **No connection lifecycle** in the palette — connecting/disconnecting stays in the sidebar

### What is not in the palette

Connection management, schema editing, settings. The palette is navigation only.

---

## 7. Query History & Saved Queries (QueryStore)

### File layout in `.based/`

```
.based/
  config.toml               ← VCS — connection configs
  queries.toml              ← VCS — saved queries with metadata
  vars.toml                 ← VCS — variable definitions
  local/                    ← gitignored (auto-.gitignore)
    history.jsonl           ← local — append-only, last 500 per connection
    workspace.json          ← local — open tabs, layout, scroll
```

`.based/local/` is gitignored via a `.based/.gitignore` file created on workspace init.

### queries.toml format

```toml
[[query]]
id = "q_3f8a1c"
name = "Active users 30d"
connection = "prod-pg"
tags = ["users", "retention"]
sql = """
SELECT id, email, last_login
FROM users
WHERE last_login > now() - interval '30 days'
"""

[[query]]
id = "q_9b2e4d"
name = "Signup pipeline"
connection = "analytics-mongo"
tags = ["signups"]
pipeline = """
[{"$match": {"type": "signup"}}, {"$group": {"_id": "$plan", "count": {"$sum": 1}}}]
"""
```

SQL queries use `sql` key. MongoDB pipelines use `pipeline` key (JSON array string). No `created_at`, no description — not needed for v1.

### history.jsonl format

```jsonl
{"conn":"prod-pg","query":"SELECT * FROM users LIMIT 50","ran_at":"2026-05-01T10:22:00Z","duration_ms":12,"row_count":50}
{"conn":"analytics-mongo","query":"[{\"$match\":{\"type\":\"signup\"}}]","ran_at":"2026-05-01T11:00:00Z","duration_ms":34,"row_count":120}
```

`query` field holds SQL text for SQL engines and pipeline JSON string for MongoDB — unified field name avoids ambiguity. Append-only. Capped at 500 entries per connection — oldest pruned on overflow. Never committed to VCS.

### History UX

- Shown as a **collapsible sidebar within QueryEditor** (toolbar button toggle)
- Scoped to the tab's connection — no cross-connection noise
- Click entry → loads into editor (does not run)
- Star entry → prompts for name inline → writes to `queries.toml`
- Filter: All / Saved ★ / Today

### QueryStore as a GPUI entity

`Entity<QueryStore>` initialized at app start. File I/O via Tokio bridge. `CommandPalette` and `QueryEditor` subscribe to it directly.

---

## 8. Variable System

Existing `project/variables.rs` has the substitution logic. Two changes to wire it:

1. **Substitute `$VAR` tokens** before sending query text to the engine
2. **Show a collapsible variables footer** in QueryEditor listing available vars from `.based/vars.toml`

No new data format. No UI to edit variables — the footer shows a read-only list with an "Edit file" link that opens `.based/vars.toml` in the system editor.

---

## 9. Engine Panels — v1 Scope

### Status legend

- `core` — ships in v1
- `power` — ships when core is solid
- `fix stub` — implementation exists, needs completion
- `deferred` — explicitly out of v1

### PostgreSQL

| Panel | Status | Key changes |
|-------|--------|-------------|
| Connection Dashboard | core · fix stub | Fix hardcoded placeholders, wire QueryStore for history |
| Data Viewer | core · fix stub | Wire filter bar, add cell detail overlay, render JSONB expandable |
| SQL Query Editor | core · fix stub | Add syntax highlighting, wire QueryStore + variables, history sidebar |
| Schema Inspector | core · fix stub | Complete stub render: Columns · Indexes · Constraints · Stats tabs |
| Explain Viewer | power · refactor | Parse EXPLAIN ANALYZE into visual node tree; bottleneck highlighting |
| Live Monitor | **deferred** | Out of v1 scope |

### SQLite

| Panel | Status | Key changes |
|-------|--------|-------------|
| Connection Dashboard | core · fix stub | Wire file size + PRAGMA summary, QueryStore history |
| Data Viewer | core · fix stub | Wire filter bar, cell detail, BLOB rendering |
| SQL Query Editor | core · fix stub | Syntax highlighting, QueryStore + variables, history sidebar |
| Schema Inspector | core · fix stub | Complete stub: Columns · Indexes · DDL tabs |
| Explain Query Plan | power · refactor | Parse into visual tree; highlight full table scans |
| PRAGMA Browser | power · fix stub | Complete render, add plain-English descriptions per PRAGMA |

### MongoDB

| Panel | Status | Key changes |
|-------|--------|-------------|
| Connection Dashboard | core · fix stub | Fix placeholders, wire QueryStore |
| Document Viewer | core · fix stub | Wire filter bar, add JSON tree view mode, render ObjectId/Date as chips |
| Pipeline Builder | core · fix stub | Wire QueryStore (save pipelines), per-stage preview, stage type selector |
| Collection Inspector | core · fix stub | Complete stub: Stats · Indexes · Validation tabs |
| Document Editor | core · fix stub | Implement JSON editor, validation before write, before/after diff |
| Change Stream Monitor | **deferred** | Out of v1 scope |

### Cross-engine consistency matrix

| Capability | Postgres | SQLite | MongoDB |
|------------|----------|--------|---------|
| Connection Dashboard | ✓ core | ✓ core | ✓ core |
| Data / Document Viewer | ✓ core | ✓ core | ✓ core |
| Filter bar | ✓ core | ✓ core | ✓ core (find filter) |
| Schema / Collection Inspector | ✓ core | ✓ core | ✓ core |
| Query / Pipeline Editor | ✓ core (SQL) | ✓ core (SQL) | ✓ core (pipeline) |
| Query history + saved queries | ✓ core | ✓ core | ✓ core |
| Variable substitution | ✓ core | ✓ core | ✓ core |
| Explain / Query Plan | ⚡ power | ⚡ power | — n/a |
| Cell detail overlay | ✓ core | ✓ core | ✓ core (JSON tree) |
| Write (insert/edit) | ⚡ via SQL editor | ⚡ via SQL editor | ✓ Document Editor |
| Engine-specific panel | — | ⚡ PRAGMA Browser | ✓ Pipeline Builder |
| Live monitoring | deferred | — n/a | deferred |

**Write story for Postgres and SQLite:** INSERT/UPDATE/DELETE go through the SQL Query Editor. No inline row editing in the Data Viewer. The developer always knows exactly what SQL is executing. MongoDB gets a Document Editor because JSON documents don't have a natural SQL equivalent.

---

## 10. Shared Widgets — Gap Closure

| Widget | File | What to implement |
|--------|------|-------------------|
| Code editor | `widgets/code_editor.rs` | Syntax-highlighted text input; SQL mode and JSON mode; token colorizer using GPUI text rendering |
| Filter bar | `widgets/filter_bar.rs` | Column selector + operator + value input; generates WHERE clause (SQL) or filter doc (Mongo) |
| Cell detail | `widgets/cell_detail.rs` | Overlay on cell click; raw value, type, copy button; JSON pretty-print for JSONB/document fields |
| Settings window | `settings_window/mod.rs` | Theme toggle, sidebar width, default page size, query timeout |
| File watcher | `project/watcher.rs` | Wire into Project entity — config reload on `.based/` changes without restart |

---

## 11. Gaps Explicitly Deferred

- Live monitoring (Postgres pg_stat_activity, MongoDB change streams)
- FTS5 console (SQLite) — panel exists, keep as-is
- ATTACH DATABASE (SQLite) — `attach_workspace.rs` stub, keep deferred
- Syntax highlighting via tree-sitter — use simpler token colorizer for v1; tree-sitter is a v2 polish item
- Mutations UI in Data Viewer (row-level insert/edit for SQL engines)
- Pop-out windows (`pop_out.rs`) — framework exists, not wired to panels
- Functions browser (Postgres) — v2

---

## 12. `.based/` Workspace Format

```
.based/
  config.toml         ← connections (team VCS)
  queries.toml        ← saved queries with metadata (team VCS)
  vars.toml           ← variable definitions (team VCS)
  .gitignore          ← auto-created, contains "local/"
  local/
    history.jsonl     ← query history (local only)
    workspace.json    ← open tabs + layout (local only)
```

No existing standard to interop with (DataGrip XML, Beekeeper SQLite, TablePlus binary — none worth targeting). Based owns this format. Plain TOML and JSONL — readable in any editor, diffs cleanly in git, sharable with teammates.
