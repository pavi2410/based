# P0 Validation Checklist

Workflow-based manual QA for stable-default release. Run on **macOS** plus at least one of **Linux** or **Windows**.

**Environment:** Postgres 14+ (Docker `docker compose up -d` or local). Sample DB with 2 schemas, 20+ tables, 100k+ row table for perf spot-check.

**Legend:** ☐ Not run · ✅ Pass · ❌ Fail (file bug with severity from [release-readiness.md](./release-readiness.md))

---

## 0. Prerequisites

| # | Step | Result | Notes |
|---|------|--------|-------|
| 0.1 | Fresh install or clean user data dir | ☐ | |
| 0.2 | Postgres reachable at `localhost:15432` (Docker Compose) | ☐ | |
| 0.3 | App launches without panic | ☐ | |

---

## 1. Activation (Welcome → first query)

*Refs: Track A, F (Welcome), Track C*

| # | Given | When | Then | Result |
|---|-------|------|------|--------|
| 1.1 | First launch, no saved connections | App opens | Welcome visible with New Connection CTA | ☐ |
| 1.2 | Welcome shown | Click New Connection | Connection wizard opens | ☐ |
| 1.3 | Valid local credentials | Test Connection | Success + version + latency | ☐ |
| 1.4 | Test succeeded | Connect (save profile on) | Explorer shows schemas | ☐ |
| 1.5 | Connected | New query tab + `SELECT 1` + Run | One row in results; status Success | ☐ |
| 1.6 | — | Quit and relaunch (with session) | Skips Welcome; restores connection/tabs | ☐ |

**Track acceptance:** A-AC1–4, C-AC1, F functional reqs

---

## 2. Schema workflow (explorer → DDL → refresh)

*Refs: Track B*

| # | Given | When | Then | Result |
|---|-------|------|------|--------|
| 2.1 | Connected | Expand schema → Tables | Table list appears &lt; 2s (local) | ☐ |
| 2.2 | Tables visible | Search partial table name | Matching nodes shown | ☐ |
| 2.3 | Table selected | Open data | Data tab with rows | ☐ |
| 2.4 | Table selected | Open DDL | DDL tab with definition | ☐ |
| 2.5 | External client creates new table | Refresh explorer | New table visible | ☐ |
| 2.6 | SQL tab open | Refresh explorer | SQL tab still open | ☐ |

**Track acceptance:** B-AC1–6

---

## 3. Query workflow (run modes, cancel, format, variables)

*Refs: Track C*

| # | Given | When | Then | Result |
|---|-------|------|------|--------|
| 3.1 | SQL editor | Run `SELECT 1` | Success, duration shown | ☐ |
| 3.2 | Two statements in editor | Select first only + Run | Only first executes | ☐ |
| 3.3 | Two statements | Run full script | Both execute in order | ☐ |
| 3.4 | `SELECT pg_sleep(30)` | Run then Cancel | Cancelled; UI responsive | ☐ |
| 3.5 | Invalid SQL | Run | Error status + message, no hang | ☐ |
| 3.6 | Query with `{{$randomUUID}}` | Run twice | Different UUIDs in results | ☐ |
| 3.7 | Query with `{{missing}}` | Run | Blocked with clear error | ☐ |
| 3.8 | Messy SQL | Format | Valid formatted SQL | ☐ |

**Track acceptance:** C-AC1–8

---

## 4. Data workflow (edit, save, discard, guard)

*Refs: Track D*

| # | Given | When | Then | Result |
|---|-------|------|------|--------|
| 4.1 | Table data tab | Edit cell | Unsaved changes banner | ☐ |
| 4.2 | Dirty | Save | DB updated; banner cleared | ☐ |
| 4.3 | Dirty | Discard | Original value restored | ☐ |
| 4.4 | Dirty | Close tab → Cancel on prompt | Tab remains, still dirty | ☐ |
| 4.5 | Dirty | Close tab → Discard | Tab closes, no DB change | ☐ |
| 4.6 | Row selected | Delete + Save | Row removed in DB | ☐ |
| 4.7 | — | Add row + Save | Row inserted | ☐ |
| 4.8 | Large table | Scroll / page | No freeze (subjective: acceptable) | ☐ |
| 4.9 | Result grid | Export CSV | File opens correctly | ☐ |

**Track acceptance:** D-AC1–9

---

## 5. History workflow (search, rerun, pin)

*Refs: Track E*

| # | Given | When | Then | Result |
|---|-------|------|------|--------|
| 5.1 | After several runs | Open history pane | Entries listed with metadata | ☐ |
| 5.2 | History open | Search substring | List filters | ☐ |
| 5.3 | Entry selected | Rerun | Query executes with new results | ☐ |
| 5.4 | Entry selected | Open in tab | SQL in editor, not auto-run | ☐ |
| 5.5 | Entry selected | Pin | Starred; survives restart | ☐ |
| 5.6 | Restart app | Open history | Prior entries present | ☐ |

**Track acceptance:** E-AC1–8

---

## 6. Connection errors (taxonomy spot-check)

*Refs: Track A error taxonomy*

| # | Scenario | Expected category | Result |
|---|----------|-------------------|--------|
| 6.1 | Wrong password | AUTH_FAILED | ☐ |
| 6.2 | Wrong host | NET_UNREACHABLE / TIMEOUT | ☐ |
| 6.3 | SSL disable on require-SSL server | TLS_REQUIRED | ☐ |

---

## 7. Cross-cutting

| # | Check | Result |
|---|-------|--------|
| 7.1 | ⌘K / Ctrl+K opens command palette | ☐ |
| 7.2 | Core actions discoverable in palette | ☐ |
| 7.3 | Tab shows connection/database context | ☐ |
| 7.4 | No plaintext password in logs (spot-check) | ☐ |
| 7.5 | Unsigned build: install from release artifact (smoke) | ☐ |

---

## 8. M1 foundation (Track A + Track F)

*Refs: [track-a-connections.md](./track-a-connections.md), [track-f-workspace-model.md](./track-f-workspace-model.md), [workspace-ui-contract.md](./workspace-ui-contract.md)*

| # | Given | When | Then | Result |
|---|-------|------|------|--------|
| 8.1 | Fresh app data dir | Launch app | Default workspace created in SQLite metadata store | ☐ |
| 8.2 | Main window open | Inspect top bar | Workspace + environment selectors visible | ☐ |
| 8.3 | Environment picker | Select `No Environment` | Queries still run; no env vars applied | ☐ |
| 8.4 | Custom environment exists | Switch environment | Future runs use new env scope only | ☐ |
| 8.5 | Loose query lane | Create loose query | Query appears in sidebar lane | ☐ |
| 8.6 | Loose query exists | Move to collection (context menu) | Query appears under collection | ☐ |
| 8.7 | Collection query exists | Move to loose queries | Query returns to loose lane | ☐ |
| 8.8 | Postgres wizard connect | Save connection | Template stored in workspace (password in keychain if literal) | ☐ |
| 8.9 | Connected + tabs open | Quit and relaunch | Workspace, environment, connection focus, and tabs restore | ☐ |
| 8.10 | Command palette | Search `workspace` / `environment` | New loose query, new collection, No Environment actions listed | ☐ |

**Track acceptance:** A-AC1–4 (session/connect), F-AC1–6, WUI-AC1–5

**Storage checks:**

| # | Check | Result |
|---|-------|--------|
| 8.S1 | `~/Library/Application Support/based/metadata.db` exists (macOS) | ☐ |
| 8.S2 | DB uses WAL mode (`PRAGMA journal_mode` → `wal`) | ☐ |
| 8.S3 | Literal template passwords not present in SQLite rows | ☐ |

---

## Sign-off

| Role | Name | Date | Pass? |
|------|------|------|-------|
| Dev | | | |
| QA / Founder | | | |

**Release blocked if:** any ❌ marked Critical/Blocker per [release-readiness.md](./release-readiness.md), or activation/schema/query/data/history section has &gt; 1 failed P0 case.
