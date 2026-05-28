# Workspace & Environment UI Contract (P0)

Defines top-level interaction behavior for workspace/environment controls and query movement between loose lane and collections.

## Top bar contract

## Controls

| Control | Behavior |
|---------|----------|
| Workspace switcher | Shows active workspace; switching updates explorer/query context |
| Environment picker | Shows `No Environment` plus user-defined environments |
| Connection selector | Uses current workspace’s templates |
| New Query | Creates loose query by default |

## Rules

- Workspace and environment controls must always be visible in main window chrome.
- Environment selection affects future execution only.
- Switching workspace never silently discards dirty tabs; prompt with `Save / Discard / Cancel`.

## Left pane contract

| Pane section | Purpose |
|--------------|---------|
| Loose Queries | Fast ad-hoc lane for unsorted/temporary SQL |
| Collections | Structured, reusable query groups |
| Explorer | Schema/object browse and actions |

### Move interactions

- Loose -> Collection:
  - Action: `Move to collection` (or `Save to collection`).
  - Result: query appears in selected collection.
- Collection -> Loose:
  - Action: `Move to loose queries`.
  - Result: query removed from collection and appears in loose lane.

P0 can use move semantics first; copy/duplicate can be P1.

## Workspace switch flow

1. User opens workspace dropdown.
2. Chooses another workspace.
3. If dirty tabs exist, show guard modal.
4. On confirm, switch context and refresh left pane + tab badges.

## Acceptance criteria

- [ ] **WUI-AC1:** Top bar shows workspace and environment controls on all primary workspace screens.
- [ ] **WUI-AC2:** Picker includes `No Environment` and it is selectable at any time.
- [ ] **WUI-AC3:** Switching environment does not rewrite editor contents.
- [ ] **WUI-AC4:** Move loose query to collection succeeds in <= 2 clicks.
- [ ] **WUI-AC5:** Move collection query back to loose lane succeeds in <= 2 clicks.
- [ ] **WUI-AC6:** Switching workspace with dirty tab shows confirmation dialog.
- [ ] **WUI-AC7:** Query tab context badges reflect active workspace/environment after switch.

## Notes for implementation

- Reuse existing dock/tab infrastructure and avoid new detached windows in P0.
- Add command palette parity for all primary UI actions:
  - `Switch Workspace`
  - `Select Environment`
  - `Move Query to Collection`
  - `Move Query to Loose Queries`
