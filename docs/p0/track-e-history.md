# Track E: Query History & Favorites

**Goal:** Users find, rerun, and pin past queries quickly; history survives restarts.

**Primary code:** `query_store/history.rs`, `workspace/chrome/panes/history_pane.rs`, `query_store/mod.rs`

## Functional requirements

| ID | Requirement | Priority |
|----|-------------|----------|
| E-F1 | Record each executed query with: SQL text, `conn_id`, database, timestamp, duration_ms, row_count, status (ok/error) | P0 |
| E-F2 | Persist history to disk; survive app restart | P0 |
| E-F3 | History pane: searchable list (SQL text + optional label) | P0 |
| E-F4 | **Rerun** — execute immediately in context | P0 |
| E-F5 | **Open in new tab** — populate SQL editor without auto-run | P0 |
| E-F6 | **Pin / favorite** — starred entries sort to top or separate section | P0 |
| E-F7 | Scope filter: by connection, by database | P0 |
| E-F8 | Retention cap (e.g. 500 per connection) with LRU trim | P0 (exists) |
| E-F9 | Clear history (connection-scoped + global) | P1 |

## Non-functional requirements

| ID | Requirement |
|----|-------------|
| E-NF1 | Load and render 500 entries &lt; 300ms |
| E-NF2 | Append on run is async/non-blocking |
| E-NF3 | Secrets policy: optional mask literals matching `password=` patterns P1 |
| E-NF4 | History file corruption recovers to empty + log warning |

## Data model

### HistoryEntry (extend current)

```rust
// Conceptual — align with query_store/history.rs
struct HistoryEntry {
    id: Uuid,                    // NEW: stable id for pin/delete
    conn_id: ConnectionId,
    database: Option<String>,    // NEW
    query: String,
    ran_at: OffsetDateTime,
    duration_ms: u64,
    row_count: Option<u64>,
    status: RunStatus,           // NEW: Ok | Error
    error_summary: Option<String>, // NEW when Error
    pinned: bool,                // NEW
    label: Option<String>,       // NEW user rename
}
```

### Storage layout (P0 target)

| Store | Path (global P0) |
|-------|------------------|
| History | `{data_dir}/based/history.jsonl` |
| Pinned index | embedded `pinned: true` on entry |

Migration: import existing `.based/local/history.jsonl` when project opens (one-time merge optional P1).

## UI: History pane

```
┌─────────────────────────────┐
│ [Search queries…]           │
│ Connection: [All ▼]         │
├─────────────────────────────┤
│ ★ SELECT * FROM users …     │
│   2m ago · local · 12ms · 42│
│ SELECT count(*) FROM …      │
│   1h ago · staging · error  │
├─────────────────────────────┤
│ [Rerun] [Open tab] [★ Pin]  │
└─────────────────────────────┘
```

### List item display

- Line 1: truncated SQL (80 chars) or user label if set
- Line 2: relative time · connection label · database · duration · row count or “Error”
- Pinned: star icon + pinned section at top

### Actions

| Action | Behavior |
|--------|----------|
| Rerun | Resolve connection; if disconnected prompt reconnect; run SQL in new or current editor tab |
| Open in tab | New SQL tab with text; cursor at end; no auto-run |
| Pin | Toggle `pinned`; persist immediately |
| Double-click row | Default = Open in tab (configurable P1) |

## Integration with Track C

- On run complete (success or error): `QueryStore::push_history(entry)`.
- On rerun from history: pass through variable resolver with current variable context.
- Failed runs still recorded (status Error) for debugging.

## Acceptance criteria

- [ ] **E-AC1:** After run, entry appears in history pane within 1s.
- [ ] **E-AC2:** Restart app; history entries still listed.
- [ ] **E-AC3:** Search filters list by substring in SQL.
- [ ] **E-AC4:** Rerun executes and shows new result.
- [ ] **E-AC5:** Open in tab loads SQL without executing.
- [ ] **E-AC6:** Pin moves entry to starred section; persists after restart.
- [ ] **E-AC7:** Filter by connection hides other connections’ entries.
- [ ] **E-AC8:** User finds and reruns a query in &lt; 10s (usability test).

## Gap analysis (current codebase)

| Item | Status |
|------|--------|
| HistoryEntry + jsonl persist | Implemented (missing status, pin, database, id) |
| History pane | Implemented (`history_pane.rs`) |
| Per-conn cap 500 | Implemented |
| Global store path | Project-local `.based/local/` — migrate to user data dir |
| Favorites/pin | Not implemented |
| Rerun / open tab | Verify wiring from pane |
| Error runs in history | Verify push on failure |

## Implementation notes

- `QueryStore` holds `Entity<QueryHistory>`; notify pane on push.
- Toggle history: `ToggleHistoryPane` (`bindings.rs` already bound).
- Add tests in `history.rs` for trim, pin, corrupt file recovery.
