# P0 PRD: Postgres-First Daily Driver Parity

**Status:** Draft for stable-default release  
**Primary engine:** PostgreSQL  
**Audience:** Backend/full-stack developers using Postgres daily (local, staging, occasional prod)

## Problem

Developers need a fast, native database client for everyday Postgres work—connect, explore schema, run SQL, inspect/edit rows, and reuse recent queries—without account friction or legacy admin-suite bloat.

## Solution

Ship **Based P0** as a stable-default desktop app with:

1. Reliable Postgres connection management (SSL, test-before-connect).
2. Responsive schema explorer with search and manual refresh.
3. Productive SQL editor (run modes, formatting, autocomplete baseline, variables).
4. Safe data editing (explicit save/discard, no silent commits).
5. Persistent query history and favorites.

## Goals (outcomes)

| Goal | User outcome |
|------|----------------|
| Connect | Connect to local/remote Postgres from UI in &lt; 30s |
| Explore | Find any object and open DDL or data quickly |
| Query | Run ad-hoc SQL with clear status and cancellable long runs |
| Edit | Change rows with confidence (dirty state + save/discard) |
| Reuse | Find and rerun a prior query in &lt; 10s |

## Scope (P0)

### In scope

- **Track A:** Global Postgres connection profiles, test connection, SSL modes, session/tab restore where safe.
- **Track B:** Schema/object tree (schemas, tables, views, indexes, functions), search, refresh, open data/DDL.
- **Track C:** Multi-tab SQL editor, syntax highlighting, format, run selected/current/all, cancel, autocomplete baseline, variable injection (`{{$…}}` + user vars).
- **Track D:** Result grid (sort/filter/copy, pagination), CSV/JSON export, table data tab with inline edit + save/discard.
- **Track E:** Local query history (searchable), pin/favorite, rerun.
- **Welcome:** In-window empty state / first-run; skip for returning users with restorable session.
- **Command palette:** Core actions (new query, connect, refresh, history, format, run).

### Out of scope

- MongoDB / SQLite parity depth.
- Project/workspace-scoped connections (team repos).
- VCS-backed connection or query sharing (`.based` optional, not blocking).
- Access brokering, SSO, enterprise policy.
- AI-assisted SQL.
- Full backup/restore UI.
- SSH tunnel (P0 stretch — spec in Track A; implement when ready).

## Product principles

1. **Daily-critical over edge-complete** — ship what users touch every day.
2. **Safety before speed on writes** — no implicit commits.
3. **Fast read path** — explorer and grids stay responsive on realistic DB sizes.
4. **Power-user velocity** — minimize launch friction; reopen last session by default.
5. **Postgres depth first** — one engine done well.

## UX surfaces (summary)

- **Main window:** top bar, explorer (left), tabbed center (SQL / data / DDL / history), execution panel (bottom).
- **Connection flow:** wizard/modal attached to main window (not separate app).
- **Welcome:** optional panel in main window for first-run / `Open Welcome` command.

See [track-a](./track-a-connections.md) through [track-e](./track-e-history.md) for detailed behavior.

## Data ownership (P0)

| Asset | Storage (target) | Notes |
|-------|------------------|-------|
| Connection profiles | Global user data dir (encrypted credentials) | Migrate from project-only config over time |
| Query history | Global `history.jsonl` (or equivalent) | Today: `.based/local/` when project open |
| Favorites / snippets | Global store | Pin state on history entries |
| Session state | `workspace.json` + prefs | Tabs, last connection, split ratios |

P0 allows current project-local paths while APIs are designed for global promotion (stable IDs).

## Quality gates (stable-default)

- Every P0 track has passing acceptance criteria in its spec doc.
- [validation-checklist.md](./validation-checklist.md) executed on macOS + one of Linux/Windows before release.
- No P0 write path without explicit save/discard confirmation.
- No release with P0 **Blocker** or **Critical** bugs open (see [release-readiness.md](./release-readiness.md)).

## Success metrics

| Metric | Target (directional) |
|--------|----------------------|
| Time to first successful query | &lt; 3 min from install (happy path) |
| Weekly active query users | Growth week-over-week post-launch |
| Query run success rate | High (exclude user SQL syntax errors) |
| Data edit completion rate | High; low abandoned dirty sessions |
| Week-2 retention | Users who connected once return within 14 days |

## Risks

| Risk | Mitigation |
|------|------------|
| Scope creep | Out-of-scope list + gatekeeper review per PR |
| Large schema/result perf | Lazy tree, pagination, virtualization |
| Write-safety bugs | Mandatory dirty-state contract; validation checklist |
| Global vs project storage split | Stable IDs; document migration path in Track A/E |

## Related documents

- [track-a-connections.md](./track-a-connections.md)
- [track-b-explorer.md](./track-b-explorer.md)
- [track-c-sql-editor.md](./track-c-sql-editor.md)
- [track-d-results-data.md](./track-d-results-data.md)
- [track-e-history.md](./track-e-history.md)
- [validation-checklist.md](./validation-checklist.md)
- [release-readiness.md](./release-readiness.md)
- [welcome-and-shell.md](./welcome-and-shell.md)
