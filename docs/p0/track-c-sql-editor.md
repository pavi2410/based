# Track C: SQL Editor & Execution

**Goal:** Daily SQL workflow—write, format, run with clear semantics, cancel long queries, substitute variables, see results and errors.

**Primary code:** `postgres/query_editor.rs`, `widgets/sql_editor.rs`, `postgres/grammar.rs`, `project/variables.rs`, `bindings.rs`

## Functional requirements

| ID | Requirement | Priority |
|----|-------------|----------|
| C-F1 | Multi-tab SQL editor; each tab bound to one `ConnectionId` + database context | P0 |
| C-F2 | PostgreSQL syntax highlighting | P0 |
| C-F3 | Format SQL (whole document + selection) | P0 |
| C-F4 | Run **selection** if non-empty; else run **current statement** | P0 |
| C-F5 | Run **full script** (all statements sequentially or as single batch — document choice) | P0 |
| C-F6 | Keyboard shortcuts for run modes + format | P0 |
| C-F7 | Autocomplete: SQL keywords + schema objects (tables/columns) from cache | P0 |
| C-F8 | Snippets: `sel`, `ins`, `upd`, `del`, `joi` (expand on trigger) | P1 |
| C-F9 | Cancel running query without freezing UI | P0 |
| C-F10 | Execution status: Idle, Running, Success, Error, Cancelled | P0 |
| C-F11 | Bottom panel: row count, duration, affected rows, error details | P0 |
| C-F12 | Variable injection `{{$randomUUID}}`, `{{$timestamp}}`, `{{$isoTimestamp}}`, `{{$randomInt(min,max)}}` | P0 |
| C-F13 | User-defined variables: connection scope + session scope | P0 |
| C-F14 | Pre-run variable preview; block run if required variable missing | P0 |
| C-F15 | Map Postgres errors to line/column when parser can identify statement | P1 |
| C-F16 | EXPLAIN tab (JSON plan) | P1 (exists partially) |

## Non-functional requirements

| ID | Requirement |
|----|-------------|
| C-NF1 | Keystroke latency imperceptible in files up to ~10k lines |
| C-NF2 | Autocomplete popup &lt; 200ms on warm cache |
| C-NF3 | Status transitions strictly ordered: Idle → Running → terminal state |
| C-NF4 | Variable resolution deterministic per run; logged in execution metadata |
| C-NF5 | Variable engine sandboxed — no arbitrary code execution |
| C-NF6 | Editor source never mutated by substitution unless user opts “Replace in editor” |

## Execution modes

### Statement detection (current statement)

- Delimiter: `;` outside strings/comments.
- Run current = statement under cursor (between previous and next `;`).
- If cursor in whitespace between statements, run next statement.

### Run selection

- Trim selection; if empty, fall through to current statement.
- Multiple statements in selection: execute in order; stop on first error (P0 default).

### Run full script

- Split on `;` with same lexer rules; execute each non-empty statement.
- Show aggregated status or per-statement error index.

### Cancellation

- User triggers Cancel (button + shortcut when Running).
- Issue `pg_cancel_backend` / drop in-flight task via `tokio` abort.
- UI → `Cancelled`; partial results discarded unless last statement completed.

## Keyboard bindings (P0 target)

| Action | macOS | Windows/Linux |
|--------|-------|-----------------|
| Run (selection / current) | ⌘↩ | Ctrl+Enter |
| Run full script | ⌘⇧↩ | Ctrl+Shift+Enter |
| Cancel | ⌘. or Esc (when running) | Ctrl+. or Esc |
| Format document | ⌘⇧F | Ctrl+Shift+F |
| New query tab | ⌘N (via action) | Ctrl+N |
| Command palette | ⌘K | Ctrl+K |

*Wire in `bindings.rs` and document in settings/help.*

## Variable injection

### Syntax

| Token | Resolution |
|-------|------------|
| `{{$randomUUID}}` | New UUID v4 per run |
| `{{$timestamp}}` | Unix seconds string |
| `{{$isoTimestamp}}` | RFC3339 UTC |
| `{{$randomInt(a,b)}}` | Inclusive random integer |
| `{{varName}}` | User variable (connection or session scope) |

Legacy `$VAR_NAME` from `.based/vars.toml` may remain as alias during migration.

### Pre-run preview panel

- Shows resolved SQL (secrets redacted) in read-only preview.
- For `INSERT`/`UPDATE`/`DELETE`, require explicit “Run” from preview when variables present (P0 policy for write safety).
- Missing variable → inline error list; Run disabled.

### Resolution order

1. Built-in `$` functions
2. Session variables (override)
3. Connection-scoped variables
4. Project vars.toml (if project open, optional)

## UI: SQL editor tab

```
┌──────────────────────────────────────────────────────────┐
│ [conn_label] / [database]     [Run ▼] [Cancel] [Format]  │
├──────────────────────────────────────────────────────────┤
│  1 │ SELECT * FROM users                                  │
│  2 │ WHERE id = {{userId}};                               │
├─────────────────────────── resizable split ──────────────┤
│ [Results] [Messages] [Explain]          status: Running… │
│ ┌──────────────────────────────────────────────────────┐ │
│ │ result grid                                          │ │
│ └──────────────────────────────────────────────────────┘ │
│ rows: 42  duration: 12ms                    [Export ▼] │
└──────────────────────────────────────────────────────────┘
```

### Status chips

| State | UI |
|-------|-----|
| Idle | Hidden or gray “Ready” |
| Running | Spinner + Cancel enabled |
| Success | Green summary (rows/duration) |
| Error | Red summary + expandable details |
| Cancelled | Amber “Cancelled” |

## Acceptance criteria

- [ ] **C-AC1:** `SELECT 1` runs and shows one row in results.
- [ ] **C-AC2:** Selection-only run executes only highlighted SQL.
- [ ] **C-AC3:** Full script runs two statements in order.
- [ ] **C-AC4:** Long `pg_sleep(30)` can be cancelled; UI responsive.
- [ ] **C-AC5:** Syntax error shows Postgres message in execution panel.
- [ ] **C-AC6:** `{{$randomUUID}}` expands differently on two consecutive runs.
- [ ] **C-AC7:** Missing `{{unknown}}` blocks run with clear message.
- [ ] **C-AC8:** Format produces valid equivalent SQL for simple SELECT.
- [ ] **C-AC9:** History entry recorded on successful run (Track E).

## Gap analysis (current codebase)

| Item | Status |
|------|--------|
| Query editor + run | Implemented (`query_editor.rs`) |
| Result grid in editor | Implemented |
| Explain tab | Partial |
| Run selection/current/all | Verify statement splitter |
| Format command | Verify / add |
| Cancel | Verify abort path |
| `{{$…}}` variables | Not implemented — `$VAR` in vars.toml only |
| Global keybindings for run | Partial — `bindings.rs` lacks run keys |
| Autocomplete schema objects | Verify depth |
| Variable preview panel | Not implemented |

## Implementation notes

- Extract `sql_statement_at_cursor(sql, offset) -> Range` in `postgres/grammar.rs`.
- `VariableResolver::resolve(sql, ctx) -> Result<ResolvedQuery>` in new `query/variables.rs`.
- Run pipeline: resolve → execute via `execute_sql` → push `HistoryEntry`.
