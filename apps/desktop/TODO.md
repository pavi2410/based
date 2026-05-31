# Desktop TODO

## Open Project follow-ups

Tracked after GUI Open Project (in-place switch + new-process window). Data model for recents lands in this PR; UI items below are deferred.

- [ ] **Open Recent submenu** — File menu / topbar project dropdown / Welcome; wire to `recent_projects` in `NativePreferences` (cap ~10, MRU, skip missing paths)
- [ ] **New Project wizard** — scaffold `.based/` (`project.toml`, `connections/`, `queries/`, `.env` template); offer from Welcome alongside Open Project
- [ ] **Per-project session restore** — key `SessionSnapshot` in `based-storage` by project path instead of process-global session; restore tabs on in-place switch
- [ ] **True multi-window single-process** — move `ProjectContext`, `QueryStore`, `WorkspaceRef` off process globals so each window owns a project without spawning a new process
