# Welcome & Main Shell (P0)

Supplement to PRD for startup and window layout. **Code:** `workspace/welcome.rs`, `workspace/chrome/layout.rs`, `app/shell.rs`

## Welcome view

### When to show

| Condition | Behavior |
|-----------|----------|
| First launch (no user data) | Show Welcome as initial center tab |
| No saved connections AND no restorable session | Show Welcome |
| Returning user with session snapshot | **Do not** show Welcome; restore workspace |
| User runs `Open Welcome` command | Show/focus Welcome tab |

### Content (P0)

- Product title + one-line value prop
- **New Connection** → opens Postgres wizard tab/panel
- **Recent connections** list (last 5 by `last_connected_at`) — click to connect
- **Open last session** (if snapshot exists)
- Hint: command palette shortcut (`⌘K` / `Ctrl+K`)

### Non-goals

- Separate native window for Welcome
- Marketing carousel or account sign-in

## Main window layout

```
┌─────────────────────────────────────────────────────────────────┐
│ Title bar / Top bar: connection selector · New Query · ⌘K       │
├──────────┬──────────────────────────────────────────┬───────────┤
│ Activity │                                          │ Inspector │
│ rail     │  Center tabs (SQL / Data / DDL / …)     │ (optional)│
│          │                                          │           │
│ Explorer │                                          │ History   │
│          ├──────────────────────────────────────────┤ (toggle)  │
│          │ Execution panel (status, errors, metrics)│           │
└──────────┴──────────────────────────────────────────┴───────────┘
```

## Top bar actions (P0)

| Action | Result |
|--------|--------|
| Connection dropdown | List profiles + connect/disconnect |
| New Query | New SQL tab for active connection |
| Disconnect | Close pool; explorer idle state |

## Command palette (P0 minimum)

| Command | Action |
|---------|--------|
| New Query | SQL tab |
| Connect / New Connection | Wizard |
| Refresh Metadata | Explorer cache invalidate |
| Open Welcome | Welcome tab |
| Toggle History | History pane |
| Format SQL | Active SQL tab |
| Run Query | Active SQL tab (current/selection) |

*Extend `command_palette/mod.rs` registry as features land.*

## Session snapshot (restore)

Persist per window:

- `active_connection_id`
- Open tab list: `{ kind, conn_id, payload }` (SQL text, table ref, DDL ref)
- Optional: split ratios, last focused tab

Load order on startup:

1. Load global profiles
2. If session exists → reconnect active connection async
3. Restore tabs (SQL from snapshot; data tabs refetch)
4. Else → Welcome

## Acceptance

- [ ] Returning user lands in last workspace without Welcome
- [ ] First-time user sees Welcome and can reach first query via New Connection
- [ ] `Open Welcome` available from palette after dismissing Welcome
