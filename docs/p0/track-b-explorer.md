# Track B: Schema Explorer Parity

**Goal:** Users browse and search Postgres schema quickly, refresh after external changes, and open data or DDL in tabs.

**Primary code:** `workspace/connection_tree/`, `postgres/tree.rs`, `postgres/inspector.rs`, `workspace/object_info.rs`

## Object coverage (P0)

| Node type | Show in tree | Lazy children | Context menu |
|-----------|--------------|---------------|--------------|
| Connection root | Yes | Schemas on expand | Refresh, Disconnect |
| Schema | Yes | Tables, views, functions on expand | Refresh |
| Tables folder | Yes | Table names | — |
| Table | Yes | Columns (optional P1) | Open data, Open DDL, Refresh |
| Views folder | Yes | View names | — |
| View | Yes | — | Open data, Open DDL |
| Indexes folder | Yes | Index names | Open DDL |
| Index | Yes | — | Open DDL |
| Functions folder | Yes | Function names | Open DDL |
| Function | Yes | — | Open DDL |

**P0 minimum:** schemas → tables/views → open table data or DDL. Indexes and functions as flat lists under schema is acceptable.

## Functional requirements

| ID | Requirement | Priority |
|----|-------------|----------|
| B-F1 | Lazy-load children on node expand | P0 |
| B-F2 | Persist expansion state per connection (session) | P1 |
| B-F3 | Global search box filters/highlight tree by object name | P0 |
| B-F4 | Manual **Refresh** on connection or schema node invalidates metadata cache | P0 |
| B-F5 | Double-click or context menu **Open data** on table/view | P0 |
| B-F6 | Context menu **Open DDL** generates or fetches `CREATE`/`VIEW` definition tab | P0 |
| B-F7 | Connection badge shows connected / connecting / error | P0 |
| B-F8 | Empty state when disconnected: prompt to connect | P0 |

## Non-functional requirements

| ID | Requirement |
|----|-------------|
| B-NF1 | First paint of explorer &lt; 500ms after connection ready (typical dev DB) |
| B-NF2 | Expand node does not block UI; show spinner on slow fetches |
| B-NF3 | Refresh does not close unrelated center tabs |
| B-NF4 | Node-level errors (e.g. permission denied on one schema) do not collapse entire tree |

## Metadata cache

### Behavior

- On connect: fetch schema list + table/view names (batched query).
- Cache keyed by `(connection_id, database)`.
- **Refresh** clears cache for scope (connection | schema | node) and refetches.
- Stale indicator optional P1: “Schema may be outdated — Refresh”.

### Invalidation triggers

| Event | Action |
|-------|--------|
| User clicks Refresh | Clear scope + refetch |
| Reconnect | Clear connection cache |
| External DDL (unknown) | User must refresh manually in P0 |

## Search semantics

- Filter mode (P0): hide non-matching branches; expand ancestors of matches.
- Match: case-insensitive substring on object name.
- Scope: all object types under active connection’s current database.
- Clear search restores previous expansion state (best effort).

## UI: Explorer panel

```
┌─────────────────────────────┐
│ [Search objects…        ] 🔍│
│ [↻ Refresh]                 │
├─────────────────────────────┤
│ ▼ my_connection (connected) │
│   ▼ public                  │
│     ▼ Tables                │
│       users                 │
│       orders                │
│     ▼ Views                 │
│       active_users          │
└─────────────────────────────┘
```

### Context menu (minimum)

| Selection | Items |
|-----------|--------|
| Table | Open data, Open DDL, Refresh |
| View | Open data, Open DDL |
| Index / Function | Open DDL |
| Schema | Refresh |
| Connection | Refresh, Disconnect |

## Tab open behavior

| Action | Tab type | Label pattern |
|--------|----------|---------------|
| Open data | Table data panel | `{schema}.{table}` |
| Open DDL | DDL/details panel | `{object} DDL` |
| Focus existing | If tab already open for same object, focus tab | — |

## Acceptance criteria

- [ ] **B-AC1:** After connect, user expands schema and sees tables within 2s on local Postgres.
- [ ] **B-AC2:** Search `user` shows all matching tables/views across schemas.
- [ ] **B-AC3:** After `CREATE TABLE` in external client, Refresh shows new table without restart.
- [ ] **B-AC4:** Open data on `users` opens data tab with rows.
- [ ] **B-AC5:** Open DDL shows readable definition (or clear error if permission denied).
- [ ] **B-AC6:** Refresh with open SQL tab does not close SQL tab.
- [ ] **B-AC7:** Disconnect grays explorer and blocks open actions with clear message.

## Gap analysis (current codebase)

| Item | Status |
|------|--------|
| Postgres tree | Implemented (`postgres/tree.rs`) |
| Connection tree UI | Implemented (`connection_tree/`) |
| Search | Verify / extend `object_list` or tree filter |
| Manual refresh | Partial — confirm cache invalidation path |
| DDL tab | Inspector / object_info paths exist |
| Global search across schemas | Verify behavior |

## Implementation notes

- Metadata service trait on `postgres` module: `list_schemas`, `list_tables`, `ddl_for(object)`.
- Explorer view model: `ExplorerNodeId` enum for stable context menus.
- Debounce search input 150ms.
