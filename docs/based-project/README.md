# The `.based` project format

This document is the **canonical specification** for Based’s git-friendly project folder. It describes how a `.based/` directory should be structured, authored, and interpreted by the Based desktop app.

Implementation may lag this spec during early development; when in doubt, **this document wins**.

## Design principles

1. **Version controlled by default** — connections (non-secret), queries, and shared project settings live in plain files you commit.
2. **One concern per file** — small diffs, fewer merge conflicts, easy code review.
3. **Secrets stay out of git** — passwords and tokens use environment variables or `.env`, never committed literals in connection files when avoidable.
4. **User preference stays local** — favorites, history, and personal UI state are not team-wide.
5. **Explicit over implicit** — query targets and connection bindings are declared in file content, not inferred from folder names.

---

## Directory layout

```text
.based/
  project.toml                 # Project manifest (committed)
  connections/
    **/*.conn.toml              # One file per connection (committed; folders optional)
  queries/
    **/*.query.toml            # One file per saved query (committed)
  .env                         # Local secrets (gitignored)
  .env.example                 # Template for required env vars (committed)
  state/                       # Per-user project preferences (gitignored)
  local/                       # Ephemeral runtime data (gitignored)
```

### Recommended `.based/.gitignore`

```gitignore
local/
state/
.env
.env.local
.DS_Store
Thumbs.db
```

---

## Committed vs local

| Path | In git? | Purpose |
|------|---------|---------|
| `project.toml` | Yes | Project name and global settings |
| `connections/**/*.conn.toml` | Yes | Connection definitions (hosts, engines, non-secret config) |
| `queries/**/*.query.toml` | Yes | Saved SQL / MongoDB pipelines |
| `.env.example` | Yes | Documents which env vars teammates need |
| `.env` | No | Secret values for this machine |
| `state/` | No | Favorites, active environment selection, other user prefs |
| `local/` | No | Query run history, session snapshots, caches |

---

## Schema versioning

Every committed Based file carries its own **`schema_version`** (integer). It declares **how to parse that file’s structure**. Bump it when fields are renamed, removed, or reorganized in breaking ways.

### Per file, not project-only

`schema_version` appears on **each file**, not only on `project.toml`:

| File | Versions |
|------|----------|
| `project.toml` | Manifest format |
| `connections/**/*.conn.toml` | Connection file format |
| `queries/**/*.query.toml` | Query file format |
| `state/*.toml` (gitignored) | Local state file format |

The numbers are **independent per file type**. Connection format v2 does not imply query format v2. When a spec change spans multiple file types, bump each affected type’s version in the changelog and migration notes.

**Why not a single version on `project.toml` only?**

- Each file is **self-describing** — you can copy one query or connection file elsewhere and still know how to parse it.
- Formats **evolve at different rates** — query `[target]` selectors may change without touching connection files.
- Loaders can read files **without** opening the manifest first.

**Authoring rule:** new files from templates always include `schema_version = 1` (or the current version for that file type). The app rejects unknown values with a clear message (“upgrade Based” or “migrate this file”).

---

## `project.toml`

The project manifest. No connections or queries here—only project identity and global settings.

```toml
schema_version = 1

name = "my-app"
description = "Database queries and connections for my-app"

[settings]
query_timeout = 30000      # milliseconds
max_result_rows = 1000
enable_query_cache = true
cache_ttl = 3600           # seconds
```

| Field | Required | Description |
|-------|----------|-------------|
| `schema_version` | Yes | Manifest format version |
| `name` | Yes | Display name in the Based UI |
| `description` | No | Short project description |
| `[settings]` | No | Global query / cache defaults |

---

## Connections (`connections/**/*.conn.toml`)

One file per connection. The **stable connection id** is derived from the file path — there is **no `id` field** in the file.

```text
connections/northwind.conn.toml           →  id = "northwind"
connections/local/northwind.conn.toml     →  id = "local/northwind"
connections/public/ebi_postgres.conn.toml →  id = "public/ebi_postgres"
```

**Id rule:** path relative to `connections/`, with the `.conn.toml` suffix removed.

Queries reference this id in `[target].connection = "northwind"`. Renaming or moving a connection file changes its id; update query targets accordingly (git diff makes this obvious).

**Why no `id` in the body?** A duplicated id that must match the path can drift when someone renames the file. The path is the single source of truth.

**Why `conn.toml` not `connection.toml`?** The `connections/` directory already provides context; the shorter suffix keeps paths readable while staying distinct from `*.query.toml`.

### Folder layout (organization only)

Both layouts are valid:

```text
# Flat
connections/local_postgres.conn.toml
connections/northwind.conn.toml

# Nested (sidebar sections — cosmetic only)
connections/local/northwind.conn.toml
connections/public/ebi_postgres.conn.toml
```

Nested folders affect **UI grouping** in the connection tree, not query matching or engine behavior. Use **`tags`** for attributes that queries match against.

### Common fields

```toml
schema_version = 1

label = "Local PostgreSQL"     # Human-readable name in the UI
engine = "postgres"            # postgres | mongodb | sqlite
tags = ["local", "dev"]        # Labels for query target matching and search

# Engine-specific fields below…
```

| Field | Required | Description |
|-------|----------|-------------|
| `schema_version` | Yes | Connection file format version |
| `label` | Yes | Display label |
| `engine` | Yes | Database family |
| `tags` | No | String labels; used by `[target]` selectors (AND matching) and UI filters |

There is **no `group` field**. A former `group = "local"` is expressed as `tags = ["local"]`. Tags are more flexible (`["public", "demo", "readonly"]`) and avoid two overlapping classification systems.

### PostgreSQL

```toml
schema_version = 1
label = "Local PostgreSQL (Docker)"
engine = "postgres"
tags = ["local", "dev"]

host = "localhost"
port = 5432
database = "based"
username = "based"
password = { env = "LOCAL_PG_PASSWORD" }
ssl = false                    # false | true (maps to driver SSL mode)
```

Inline passwords are allowed for local demos but **prefer `{ env = "VAR" }`** and document the var in `.env.example`.

### SQLite

```toml
schema_version = 1
label = "Northwind Database"
engine = "sqlite"
tags = ["local", "demo"]

file = "data/northwind.db"     # Path relative to repo root

[pragma]
journal_mode = "wal"
synchronous = "normal"
foreign_keys = true
```

| Field | Required | Description |
|-------|----------|-------------|
| `file` | Yes | Database file path (relative to repo root or absolute) |
| `[pragma]` | No | Connection-time PRAGMA settings (see below) |

#### `[pragma]` table

Optional SQLite PRAGMA overrides applied immediately after the connection pool opens. Omitted keys use Based defaults.

```toml
[pragma]
journal_mode = "wal"
synchronous = "normal"
foreign_keys = true
```

| Key | Default | Values | Applied as |
|-----|---------|--------|------------|
| `journal_mode` | `wal` | `delete`, `wal`, `truncate`, `persist`, `memory`, `off` | `PRAGMA journal_mode=…` |
| `synchronous` | `normal` | `off`/`0`, `normal`/`1`, `full`/`2`, `extra`/`3` | `PRAGMA synchronous=…` |
| `foreign_keys` | `true` | `true`, `false` | `PRAGMA foreign_keys=ON\|OFF` |

**`journal_mode`**

| Value | Typical use |
|-------|-------------|
| `wal` | **Default.** Best for desktop use — readers do not block writers |
| `delete` | Classic rollback journal; legacy databases |
| `truncate` | Rollback journal; truncates the log on checkpoint |
| `persist` | Rollback journal; header persisted after commit |
| `memory` | Journal in RAM only |
| `off` | No journal (rare) |

**`synchronous`**

| Value | Meaning |
|-------|---------|
| `off` | No syncs (`OFF` / `0`) — fastest, least durable |
| `normal` | **Default.** Sync at critical moments (`NORMAL` / `1`); good balance with WAL |
| `full` | Full sync after every write (`FULL` / `2`) |
| `extra` | Like `full` plus extra sync for WAL (`EXTRA` / `3`) |

**`foreign_keys`**

When `true` (default), Based runs `PRAGMA foreign_keys=ON` so SQLite enforces referential integrity. Set `false` only when inspecting legacy schemas without FK metadata.

**Minimal SQLite connection** (all defaults — omit `[pragma]` entirely):

```toml
schema_version = 1
label = "Northwind"
engine = "sqlite"
file = "data/northwind.db"
```

Equivalent to `journal_mode = "wal"`, `synchronous = "normal"`, `foreign_keys = true`.

**Legacy / read-only example:**

```toml
[pragma]
journal_mode = "delete"
synchronous = "full"
foreign_keys = false
```

Only `[pragma]` keys present in the file override defaults; other settings keep Based defaults.

### MongoDB

```toml
schema_version = 1
label = "Analytics MongoDB"
engine = "mongodb"
tags = ["staging", "readonly"]

url = { env = "MONGO_URL" }
# Or literal URI for public read-only demos:
# url = "mongodb+srv://user:pass@cluster.example.net"
database = "analytics"         # Default database (optional)
```

---

## Queries (`queries/**/*.query.toml`)

One file per query. The app loads **all** `*.query.toml` files under `queries/`, recursively.

### Folder layout (organization only)

Both layouts are valid and **semantically equivalent** if file contents match:

```text
# Flat (prefix by connection or topic)
queries/northwind-recent-orders.query.toml
queries/pg-list-tables.query.toml

# Nested (folders for humans / UI collections)
queries/northwind/recent-orders.query.toml
queries/reports/northwind/revenue.query.toml
```

**Rules:**

- **Path does not determine** which connection runs the query. Use `[target]` (below).
- **Path may inform UI grouping** — the app can derive a `collection` label from parent folder names for sidebar organization.
- **Internal identity** — use the file’s path relative to `queries/` (e.g. `northwind/recent-orders.query.toml`) to avoid stem collisions.
- Optional explicit `collection = "reports"` in the file overrides folder-derived grouping.

There is **no** monolithic `queries.toml`. Do not commit team queries there.

### Query file structure

```toml
schema_version = 1

name = "Recent Orders"
description = "Most recent orders with customer and employee names"
tags = ["orders", "reports"]
collection = "northwind"       # Optional; UI grouping only

[target]
connection = "local/northwind" # Exact connection id (path under connections/)

[sql]
query = """
SELECT o.OrderID, o.OrderDate, c.CompanyName
FROM Orders o
JOIN Customers c ON o.CustomerID = c.CustomerID
ORDER BY o.OrderDate DESC
LIMIT 50;
"""
```

| Field | Required | Description |
|-------|----------|-------------|
| `schema_version` | Yes | Query file format version |
| `name` | Yes | Display name in Saved pane / command palette |
| `description` | No | Longer explanation |
| `tags` | No | Free-form labels for search/filter |
| `collection` | No | UI grouping; defaults from parent folder if omitted |
| `[target]` | Yes | Where this query may run (see below) |
| `[sql]` or `[pipeline]` | One required | Query body |

**Favorites are not stored in query files.** Pinning a query is a per-user preference in `state/favorites.toml` (see [Local state](#local-state-state)).

### SQL queries (`[sql]`)

For `engine = "postgres"` and `engine = "sqlite"` connections.

```toml
[sql]
query = """
SELECT table_schema, table_name
FROM information_schema.tables
WHERE table_type = 'BASE TABLE'
ORDER BY 1, 2;
"""
```

**Future (optional):** external body file for very large SQL:

```toml
[sql]
file = "revenue-report.sql"    # Relative to the .query.toml directory
```

### MongoDB pipelines (`[pipeline]`)

For `engine = "mongodb"` connections.

```toml
mongo_collection = "orders"    # Required for pipeline execution

[pipeline]
query = """
[
  { "$match": { "status": "completed" } },
  { "$group": { "_id": "$plan", "count": { "$sum": 1 } } },
  { "$sort": { "count": -1 } }
]
"""
```

`query` is a JSON array string (MongoDB aggregation pipeline).

---

## Query targets (`[target]`)

The `[target]` block declares **which connection(s)** a query may run against. Resolution happens at open/run time.

### Exact connection (default)

Run only against one stable id (path under `connections/`):

```toml
[target]
connection = "local/northwind"
```

Shorthand: if the only key in `[target]` is `connection`, this is the common case for repo-specific runbooks.

### Engine family

Run against **any** connected database of that family (Postgres, MongoDB, or SQLite):

```toml
[target]
engine = "postgres"
```

Use for portable introspection queries (e.g. list tables via `information_schema`).

### Attribute selector

Match connections that satisfy **all** specified attributes (AND):

```toml
[target]
engine = "postgres"
tags = ["public", "demo"]
```

| Attribute | Matches |
|-----------|---------|
| `engine` | Connection’s `engine` field |
| `tags` | Connection must include **every** listed tag |

### Explicit allow-list

Run against one of several known connections:

```toml
[target]
any = ["northwind", "staging_sqlite"]
```

### Resolution behavior

When the user opens or runs a query:

1. **Focused connection** — if the active connection matches `[target]`, use it.
2. **Single match** in the project → use that connection.
3. **Multiple matches** → show a picker filtered to matching connections.
4. **No match** → error: *“No connection matches this query’s target.”*

The query text is never rewritten per connection; the user picks (or the app picks) a compatible connection.

---

## Local state (`state/`)

Gitignored per-user data for the project.

### `state/favorites.toml`

Queries the user has pinned. References project queries by path or id, **not** embedded SQL.

```toml
schema_version = 1

[[favorite]]
path = "northwind/recent-orders.query.toml"

[[favorite]]
path = "mindsdb/fraud-summary.query.toml"
```

Starring from the History pane writes here, not into `queries/*.query.toml`.

### Other state files (planned)

| File | Purpose |
|------|---------|
| `state/active_environment.toml` | User’s selected environment name (when environments ship) |
| `state/ui.toml` | Sidebar layout, last-open collection, etc. |

---

## Local runtime (`local/`)

Ephemeral data; never committed.

| File | Purpose |
|------|---------|
| `local/history.jsonl` | Append-only query run history (capped per connection) |
| `local/session.json` | Open tabs, cursor positions (optional) |

History entries are created when the user runs queries in the app. They are not part of the shared query library.

---

## Secrets (`.env`)

Copy `.env.example` to `.env` and fill in values locally.

```bash
# .env.example
LOCAL_PG_PASSWORD=
MONGO_URL=mongodb://localhost:37017
```

Connection files reference vars:

```toml
password = { env = "LOCAL_PG_PASSWORD" }
url = { env = "MONGO_URL" }
```

Never commit `.env`.

---

## Variables

Parameterized queries (`$VAR`, `{{name}}`, scoped environments) are **not fully specified in this document**. They will be defined in a separate spec when the vars model is finalized.

Until then:

- Do not rely on committed `vars.toml` layout in new projects.
- Query files may use literal values in SQL for demos.

---

## App behavior summary

| User action | Source |
|-------------|--------|
| Browse connections | `connections/**/*.conn.toml` |
| Open Saved query | `queries/**/*.query.toml` + `state/favorites.toml` for pins |
| Run query | Resolve `[target]` → execute `[sql]` or `[pipeline]` |
| Star a query | Write `state/favorites.toml` |
| View run history | `local/history.jsonl` |
| Reload after git pull | File watcher on `.based/` reloads connections and queries |

Queries appear in:

- **Saved** side pane (star icon in status bar)
- **Command palette** (⌘K / Ctrl+K)
- **Workspace** left pane (optional grouping by `collection` / folder)

---

## Full minimal example

```text
my-repo/
  .based/
    project.toml
    .env.example
    connections/
      local/
        northwind.conn.toml
        local_postgres.conn.toml
    queries/
      northwind/
        recent-orders.query.toml
      pg-list-tables.query.toml
  data/
    northwind.db
```

**`.based/project.toml`**

```toml
schema_version = 1
name = "my-repo"
description = "Example Based project"

[settings]
query_timeout = 30000
max_result_rows = 1000
```

**`.based/connections/local/northwind.conn.toml`**

```toml
schema_version = 1
label = "Northwind"
engine = "sqlite"
tags = ["local", "demo"]
file = "data/northwind.db"

[pragma]
journal_mode = "wal"
synchronous = "normal"
foreign_keys = true
```

**`.based/queries/pg-list-tables.query.toml`**

```toml
schema_version = 1
name = "List Tables"
description = "Postgres information_schema tables"

[target]
engine = "postgres"

[sql]
query = """
SELECT table_schema AS schema, table_name AS name
FROM information_schema.tables
WHERE table_type = 'BASE TABLE'
  AND table_schema NOT IN ('pg_catalog', 'information_schema')
ORDER BY 1, 2;
"""
```

**`.based/queries/northwind/recent-orders.query.toml`**

```toml
schema_version = 1
name = "Recent Orders"
tags = ["orders"]

[target]
connection = "local/northwind"

[sql]
query = """
SELECT OrderID, OrderDate FROM Orders
ORDER BY OrderDate DESC LIMIT 50;
"""
```

---

## Changelog (spec)

| Date | Change |
|------|--------|
| 2026-05-30 | SQLite `[pragma]` table (`journal_mode`, `synchronous`, `foreign_keys`); drop `group`; tags-only; connection id from path |
| 2026-05-30 | Initial canonical spec: `project.toml`, per-file connections and queries, `[target]` selectors, `state/favorites`, per-file `schema_version` |
