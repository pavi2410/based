# P0 Release Readiness

Criteria for promoting Based P0 from internal dogfood to **stable-default** (recommended for all new users).

## Release gates (all required)

| Gate | Criteria |
|------|----------|
| Specs complete | All track docs A–E reviewed; PRD agreed |
| Validation | [validation-checklist.md](./validation-checklist.md) 100% P0 cases pass on macOS + one other OS |
| Blockers | Zero open Blocker bugs |
| Criticals | Zero open Critical bugs |
| Write safety | All Track D navigation-guard cases pass |
| Performance smoke | Explorer + 500-row page + 1k-row result scroll acceptable on reference hardware |
| Artifacts | Signed or unsigned release per project policy; checksums published |
| Docs | README install path accurate; known limitations listed |

## Bug severity definitions

### Blocker

Release must not ship.

- App crash on launch or connect for standard Postgres setup
- Data loss: silent commit, save discards data without confirmation, or corrupts rows
- Cannot connect with valid local Postgres credentials
- Cannot run any query on connected database
- Security: secrets written to world-readable files or logged in plaintext

### Critical

Release should not ship without fix or documented workaround.

- Major feature in P0 scope completely non-functional (history, save, explorer refresh)
- Wrong data shown in grid without user indication
- Session restore loses all tabs every launch
- Cancel does not stop runaway query; requires force-quit
- Export produces corrupt files

### Major

Ship allowed with release notes; fix in next patch.

- Incorrect error category but message usable
- Pin/history search broken for subset of cases
- Format SQL breaks valid queries (edge cases)
- Performance degradation on large schemas but usable
- Missing keyboard shortcut (mouse path works)

### Minor

Backlog.

- Cosmetic UI issues
- Non-P0 engine regressions
- Typo in copy
- Wishlist features (SSH, LSP)

## Stable-default quality bar

Beyond “no blockers,” stable-default implies:

1. **First-run success:** New user completes activation flow (checklist §1) without docs.
2. **Daily loop:** Experienced user completes §2–§5 in one session without confusion.
3. **Trust:** Write path never surprises; errors are actionable.
4. **Performance:** No multi-second UI freezes on common paths (connect, expand schema, run simple SELECT).

## Rollout stages

| Stage | Audience | Entry criteria | Exit criteria |
|-------|----------|----------------|---------------|
| Internal | Maintainer | Validation started | All §1–§5 pass |
| RC | 5–20 power users | Internal pass | 1 week, no Critical+ from cohort |
| Stable-default | All downloaders | RC pass + Blocker/Critical = 0 | Ongoing monitoring |

## Known limitations (document in release notes)

- Installers may be unsigned (platform-specific warnings).
- SSH tunnel may be unavailable in P0 if not shipped.
- MongoDB/SQLite not at Postgres parity.
- Variable syntax: `{{$…}}` and migration from `$VAR` — document both if transitional.
- Global connection store migration from project `.based` config — document one-time behavior.

## Post-release monitoring (first 2 weeks)

| Signal | Action threshold |
|--------|------------------|
| Crash reports | Any Blocker pattern → hotfix |
| GitHub issues tagged `p0-regression` | Triage within 24h |
| Failed activation (qualitative feedback) | UX pass on Welcome/wizard |
| History/data complaints | Priority Track D/E fix |

## Checklist before tagging release

- [ ] Version stamped by CI (`script/set-version.py`)
- [ ] `validation-checklist.md` signed off
- [ ] Release notes: features, limitations, migration notes
- [ ] Per-asset SHA-256 digests visible on GitHub Release assets
- [ ] Critical/Blocker query on issue tracker = empty

## Reference

- [PRD.md](./PRD.md) — scope and metrics
- Track specs — per-feature acceptance IDs (A-AC*, B-AC*, …)
