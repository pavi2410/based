# Track A: Connection & Session Foundation

**Goal:** Users connect to Postgres reliably, test before save, and resume work after restart without re-entering credentials.

**Primary code:** `apps/desktop/src/connection/`, `postgres/wizard.rs`, `connection/persistence.rs`, `app/prefs.rs`

## Functional requirements

| ID | Requirement | Priority |
|----|-------------|----------|
| A-F1 | Create, edit, delete Postgres connection profiles (label, host, port, database, user, password) | P0 |
| A-F2 | SSL mode selection: `disable`, `prefer`, `require` (extend to `verify-ca` / `verify-full` when certs UI exists) | P0 |
| A-F3 | **Test connection** — independent of save/connect; shows latency + server version on success | P0 |
| A-F4 | **Connect** — establish pool/session; surface errors in wizard status area | P0 |
| A-F5 | Optional **connect without saving** profile (ephemeral session) | P0 |
| A-F6 | URI paste / `postgresql://` parse to populate fields | P1 (exists in wizard) |
| A-F7 | SSH tunnel configuration | P1 (out of initial P0 slice if not ready) |
| A-F8 | On app relaunch: restore last active connection + open tabs when safe | P0 |
| A-F9 | Connection switcher in top bar / explorer lists all profiles with connect/disconnect state | P0 |

## Non-functional requirements

| ID | Requirement |
|----|-------------|
| A-NF1 | Test/connect must not block UI thread; show `Testing` / `Connecting` within 1s |
| A-NF2 | Passwords stored via OS keychain when available; else encrypted file under user data dir |
| A-NF3 | Secrets never logged in plaintext |
| A-NF4 | Failed operations return categorized error (see taxonomy below) |

## Error taxonomy

Errors shown to users use: **category** + **summary** + **details** + **suggested action**.

| Category | Code | Typical causes | User-facing summary | Suggested action |
|----------|------|----------------|---------------------|------------------|
| Network | `NET_UNREACHABLE` | Host down, wrong host/port, firewall | Cannot reach database server | Check host/port and network/VPN |
| Network | `NET_TIMEOUT` | Slow/unresponsive server | Connection timed out | Retry or increase timeout in settings |
| Auth | `AUTH_FAILED` | Wrong user/password | Authentication failed | Verify username and password |
| Auth | `AUTH_METHOD` | Server requires cert/GSSAPI | Authentication method not supported | Change auth or use supported method |
| TLS | `TLS_REQUIRED` | Server requires SSL, client disabled | SSL required by server | Set SSL mode to Require |
| TLS | `TLS_REJECTED` | Cert mismatch, hostname | SSL handshake failed | Check SSL mode and certificates |
| Server | `DB_NOT_FOUND` | Unknown database name | Database does not exist | Fix database name or create database |
| Server | `SERVER_ERROR` | Postgres error during connect | Server rejected connection | Expand details; check server logs |
| Config | `CONFIG_INVALID` | Missing host, bad port | Connection settings incomplete | Fill required fields |
| Config | `URI_PARSE` | Malformed connection URI | Could not parse connection URI | Fix URI format |
| Tunnel | `SSH_FAILED` | SSH auth/host (when implemented) | SSH tunnel failed | Check SSH host, user, and keys |
| Internal | `INTERNAL` | Unexpected app error | Something went wrong | Retry; report if persistent |

**Mapping implementation:** `postgres/wizard.rs` `WizardStatus::TestErr` / `ConnectErr` should carry category when possible (parse `sqlx` / driver errors).

## UI: Connection wizard / modal

### Layout

- **Basic (always visible):** Label, Host, Port, Database, Username, Password
- **Advanced (collapsible):** SSL mode, URI paste field
- **Future advanced:** SSH host/port/user/key, client certs
- **Actions row:** `Test`, `Connect`, `Save & Connect` (or toggle “Save profile”)
- **Status area:** Idle | Testing | Test OK (latency, version) | Test error | Connecting | Connect error

### Interaction rules

- `Connect` disabled until host, port, database, username are non-empty.
- `Test` does not persist profile or open workspace tabs.
- Successful **Connect** adds entry to `ConnectionRegistry` and focuses explorer for that connection.
- Editing an existing profile pre-fills fields; password field may be empty = “keep existing”.

## Session restore

### Persist

| Field | Store |
|-------|--------|
| Connection IDs + last connected time | `WorkspaceState` / global session file |
| Open tab specs (SQL, table, DDL) | Session snapshot (per window) |
| Active connection ID | Session snapshot |
| Last error per connection (optional) | `PersistedConnection::last_error` |

### Restore policy

- On launch with restorable session: open main window, reconnect active connection in background, reopen tabs **read-only safe** first (SQL buffers from snapshot; data tabs refetch).
- On launch with no session: show **Welcome** (see PRD).
- If reconnect fails: show banner on connection with taxonomy error; tabs show disconnected state until user reconnects.

## Acceptance criteria

### Happy path

- [ ] **A-AC1:** New user opens Welcome → New Connection → fills local Postgres → Test succeeds → Connect → explorer shows schemas.
- [ ] **A-AC2:** Saved profile appears in connection list after “Save & Connect”.
- [ ] **A-AC3:** Quit and relaunch restores last connection and at least one SQL tab with prior text.
- [ ] **A-AC4:** Test connection shows server version string and round-trip latency.

### Error path

- [ ] **A-AC5:** Wrong password shows `AUTH_FAILED` with actionable message (no stack trace in UI).
- [ ] **A-AC6:** Wrong host shows `NET_UNREACHABLE` or `NET_TIMEOUT`.
- [ ] **A-AC7:** SSL mismatch shows `TLS_*` category.
- [ ] **A-AC8:** UI remains responsive during 10s+ test on slow network (spinner visible).

### Ephemeral connect

- [ ] **A-AC9:** User can connect with “Save profile” off; connection works but does not appear in saved list after restart (unless promoted).

## Gap analysis (current codebase)

| Item | Status |
|------|--------|
| Wizard test/connect | Implemented (`postgres/wizard.rs`) |
| SSL modes | Implemented (`SslMode`) |
| Global profiles store | Partial — tied to `.based/config.toml` project model; P0 needs global user dir |
| SSH tunnel | Not implemented |
| Ephemeral connect | Not explicit — add toggle |
| Session tab restore | Partial — `persistence.rs` tracks connections, not full tab payloads |
| Error taxonomy codes | Partial — string errors only |

## Implementation notes

- Introduce `ConnectionProfile` persisted under `dirs::data_dir()/based/profiles.json` (or SQLite) with stable UUID `profile_id`.
- `ConnectionId` at runtime maps to `profile_id` + optional ephemeral flag.
- Wizard emits `WizardEvent::Connected(PgConnection)`; registry owns lifecycle (`connection/lifecycle.rs`).
