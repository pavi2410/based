# Desktop TODO

Project-wide items: see [TODO.md](../../TODO.md) at the repo root.

## Open Project follow-ups

Tracked after GUI Open Project (in-place switch + new-process window). Data model for recents lands in this PR; UI items below are deferred.

- [ ] **Open Recent submenu** — File menu / topbar project dropdown / Home; wire to `recent_projects` in `NativePreferences` (cap ~10, MRU, skip missing paths)
- [ ] **New Project wizard** — scaffold `.based/` (`project.toml`, `connections/`, `queries/`, `.env` template); offer from Home alongside Open Project
- [ ] **Per-project session restore** — key `SessionSnapshot` in `based-storage` by project path instead of process-global session; restore tabs on in-place switch
- [ ] **True multi-window single-process** — move `ProjectContext`, `QueryStore`, `WorkspaceRef` off process globals so each window owns a project without spawning a new process

## Tab strip (gpui-component)

Upstream parity for center editor tabs. Today: Close tab in panel ⋯ menu and ⌘W / Ctrl+W; Home respawns when the last tab closes. See `CLAUDE.md` (Tab strip).

- [ ] **Per-tab × and overflow chevron** — [gpui-component](https://github.com/longbridge/gpui-component) `TabPanel` API (Tabs-demo parity); no fork in this repo. Optional release-note mention until shipped.
