# desktop-native — Phase 7 UI feedback & errors (locked scope)

This document locks **how** `apps/desktop-native` surfaces errors, validation, and async feedback. It replaces an earlier “toasts-only” framing with a **dual-layer** model aligned with current product taste: **contextual first**, **notification tray second**.

## Explicitly out of Phase 7

The following are **not** part of this phase (may land later or in the Tauri app only):

- Vim-style keyboard navigation in grids/editors (**B3**)
- Beginner / Pro mode toggle and related chrome switching
- Telemetry / tracing productization (**G1**, **G2**)

## Dual-layer model

### Layer 1 — Contextual (primary)

Use when the user can tie the failure to a **specific control or region**.

| Mechanism | Use when |
|-----------|----------|
| **Inline text** | Short, actionable messages; validation; result of “Run” in that panel |
| **Inline affordance** | Border / subtle state on the offending field or row |
| **Anchored popover or tooltip** | Long driver / server messages; “copy details”; does not replace inline summary |

**Rules**

- Pick **one primary surface** per failure (usually inline or inline + popover for depth).
- The message should **point at the source** (wizard field, query strip, connection row).

### Layer 2 — Notification tray (VS Code–style)

Use `gpui_component::Root`’s **notification list** when the event is **ambient**, **cross-surface**, or **decoupled from the current focus**.

| Use for | Examples |
|---------|----------|
| Async / background | Connect failed while another tab is focused |
| Progress | Long-running task; optional mirror if user switches away |
| Global info / success | “Export finished”, “File saved” (if not shown inline) |

**Rules**

- Do **not** rely on the tray as the **only** place for form or query errors that have a clear anchor.
- Avoid **duplicating** the same error in full detail in both inline and tray; a short tray line that **focuses** the relevant panel is acceptable later if needed.

## Surface-specific defaults (native client)

| Surface | Primary | Tray |
|---------|---------|------|
| Connection wizard | Inline + popover for long text | Optional if user leaves wizard mid-async |
| Sidebar / connection row | Status glyph + short reason; popover for detail | Connect failed while row not visible |
| Query / run panel | Inline under controls | Optional secondary |
| Status bar | Single low-detail line (optional) | N/A |

## Implementation order (Phase 7 execution)

1. **Notification API** — Thin helpers on top of `NotificationList` (info, error, optional progress) callable from `Workspace` or `Root` update paths.
2. **First vertical slice** — e.g. query editor: inline error region + optional popover for full text.
3. **Connection lifecycle** — Row-level inline/popover; tray when failure is off-screen.
4. **Conventions** — New panels follow this doc; code review checks primary vs tray usage.

## Relation to other Phase 7 work

Command palette (**⌘K**), actions registry, keybindings, theme persistence, and README/mise tasks proceed in parallel where they do not conflict. Error copy should remain **stable strings** where possible so actions (“Focus last error”) can target the right panel later.

When the Tauri app adopts the same product rules, mirror **semantics** (contextual vs tray); implementation will differ (Radix toast vs GPUI notifications).
