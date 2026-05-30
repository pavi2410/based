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
| `tags` | No | String labels; used by `[target]` (`tags` / `exclude_tags`) and UI filters |

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

# Nested (folders for humans — cosmetic only)
queries/northwind/recent-orders.query.toml
queries/reports/northwind/revenue.query.toml
```

**Rules:**

- **Path does not determine** which connection runs the query. Use `[target]` (below).
- **Nested folders** may group queries in the UI sidebar; same rationale as `connections/` — organization only, no extra field.
- **Internal identity** — the file’s path relative to `queries/` (e.g. `northwind/recent-orders.query.toml`) avoids stem collisions.

There is **no `collection` field** on query files. Use **`tags`** for labels and filtering (search, command palette), and folder layout for visual grouping.
There is **no** monolithic `queries.toml`. Do not commit team queries there.

### Query file structure

```toml
schema_version = 1

name = "Recent Orders"
description = "Most recent orders with customer and employee names"
tags = ["orders", "reports"]

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
| `tags` | No | Free-form labels on the query itself (search, filter in Saved / ⌘K) |
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

v1 uses a **flat filter stack**: fields combine with **AND**. There is no nested tag policy language and no globs in v1.

### Fields

| Field | Type | Description |
|-------|------|-------------|
| `connection` | string **or** string[] | Exact id, or one-of several ids (see below) |
| `engine` | string | `postgres`, `mongodb`, or `sqlite` |
| `tags` | string[] | Connection must have **every** listed tag |
| `exclude_tags` | string[] | Connection must have **none** of these tags |

Empty arrays for `tags` or `exclude_tags` are treated as omitted. An empty `connection` array is **invalid** at load time.

Tag matching is **case-sensitive** and compares literal strings on the connection file’s `tags` list.

### `connection`: string or array

One field, two shapes:

```toml
# Exactly one connection (exclusive — most runbooks)
[target]
connection = "local/northwind"

# One of several connection ids (OR)
[target]
connection = ["public/ebi", "public/mindsdb"]
```

| Form | Rule |
|------|------|
| **String** | Run only against this id. **Must not** appear on the same `[target]` as `engine`, `tags`, or `exclude_tags` (load error). |
| **Array** | Connection id must be **one of** the listed ids. **May** combine with `engine`, `tags`, and `exclude_tags` (all AND). |
| **`["only"]`** | Allowed; equivalent to `connection = "only"`. |

Connection ids are paths under `connections/` without the `.conn.toml` suffix (e.g. `local/northwind`).

### Filter mode (no exclusive string)

When `connection` is absent or an **array**, evaluate filters in any order (all are AND):

1. Start with all connections in the project
2. **`engine`** — keep if engine matches (when set)
3. **`tags`** — keep if connection has every listed tag (when set)
4. **`exclude_tags`** — keep if connection has none of these tags (when set)
5. **`connection` array** — keep if id is in the list (when set)

### Examples

**Portable Postgres introspection**

```toml
[target]
engine = "postgres"
```

**Public demo Postgres only**

```toml
[target]
engine = "postgres"
tags = ["public", "demo"]
```

**Postgres, not production**

```toml
[target]
engine = "postgres"
exclude_tags = ["prod"]
```

**One of several known public databases**

```toml
[target]
connection = ["public/ebi", "public/mindsdb"]
engine = "postgres"
tags = ["demo"]
```

**Single runbook query (exclusive)**

```toml
[target]
connection = "local/northwind"
```

### Resolution behavior

When the user opens or runs a query:

1. **Exclusive string** — use that connection id (fail if missing from project).
2. **Filter mode** — compute the matching set from the rules above.
3. **Focused connection** — if the active connection is in the set, prefer it.
4. **Single match** → use that connection.
5. **Multiple matches** → show a picker limited to the matching set.
6. **No matches** → error: *“No connection matches this query’s target.”*

The query text is never rewritten per connection.

### Deferred past v1

- Globs on connection ids (`public/*`)
- Tag OR (`tags_any`) and nested `[target.tags]` tables
- Cross-engine OR in one target

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
| `state/ui.toml` | Sidebar layout, query tree expansion, etc. |

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
- **Workspace** left pane (grouped by folder path under `queries/`)

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
| 2026-05-30 | Drop query `collection` field; use `tags` and folder layout only |
| 2026-05-30 | v1 `[target]`: `connection` string\|array, `engine`, `tags`, `exclude_tags`; flat AND semantics |
| 2026-05-30 | SQLite `[pragma]` table (`journal_mode`, `synchronous`, `foreign_keys`); drop `group`; tags-only; connection id from path |
| 2026-05-30 | Initial canonical spec: `project.toml`, per-file connections and queries, `[target]` selectors, `state/favorites`, per-file `schema_version` |
