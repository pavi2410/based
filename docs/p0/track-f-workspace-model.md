# Track F: Workspace, Collections, Loose Queries, Environments

**Goal:** support ad-hoc SQL and structured reuse in the same product model, without forcing repo coupling or preset environments.

## P0 entities

| Entity | Description |
|--------|-------------|
| Workspace | Logical app context (name, active environment, templates, queries) |
| Loose query | Ad-hoc query not assigned to any collection |
| Collection | Named group of reusable queries |
| Environment | User-defined context (`staging`, `perf`, `qa-us-east`, etc.) |
| `No Environment` | Default state; valid and fully supported |
| Connection template | Parameterized host/db/user/etc. with variable tokens |

## Functional requirements

| ID | Requirement | Priority |
|----|-------------|----------|
| F-F1 | Create/rename/delete workspace | P0 |
| F-F2 | Switch active workspace from top bar | P0 |
| F-F3 | Create loose query and run immediately | P0 |
| F-F4 | Create collection and move/copy loose query into it | P0 |
| F-F5 | Move collection query back to loose lane | P0 |
| F-F6 | Create/edit/delete user-defined environments | P0 |
| F-F7 | Active environment defaults to `No Environment` | P0 |
| F-F8 | Switching environment affects future runs only (no text rewrite) | P0 |
| F-F9 | Connection templates stored at workspace scope | P0 |

## Non-functional requirements

| ID | Requirement |
|----|-------------|
| F-NF1 | Workspace switch target visible and deterministic at all times |
| F-NF2 | Collection/loose query move operations are reversible |
| F-NF3 | `No Environment` must be represented explicitly in state, not as error/null edge case |
| F-NF4 | Workspace model must persist and restore without requiring git repo |

## Data model (P0)

- Workspace: `id`, `name`, timestamps, `active_environment_id`, `connection_templates`, `collections`, `loose_queries`.
- Collection: `id`, `name`, `queries[]`.
- Loose query: `id`, `name`, `sql`, optional `connection_template_id`.
- Environment: `id`, `name`, `variables`.
- Active environment: nullable id (`None` => `No Environment`).

## Acceptance criteria

- [ ] **F-AC1:** User creates workspace and lands in empty loose-query lane.
- [ ] **F-AC2:** User creates loose query, runs it, then moves it to a collection.
- [ ] **F-AC3:** User moves a collection query back to loose lane.
- [ ] **F-AC4:** User creates custom environment (`perf`) and selects it.
- [ ] **F-AC5:** User re-selects `No Environment` and queries still run.
- [ ] **F-AC6:** Restart app restores workspace and active environment selection.

## Implementation hooks

- `crates/based-workspace/src/model.rs` defines P0 workspace entities.
- `crates/based-workspace/src/resolve.rs` resolves runtime connection templates from variable scopes.
- UI wiring lands in workspace chrome/top bar and query-sidebars incrementally.
