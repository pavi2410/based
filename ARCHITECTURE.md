# Based - Architecture

This document captures the current production architecture for `based`.

## Product thesis

A **git-friendly**, **local-first** desktop database client for **Postgres, MongoDB, and SQLite**.

The project model is centered on a committed `.based/` folder:

- `config.toml` - project + connection metadata (committed)
- `.env` - local secrets (git-ignored)
- `queries/*.query.toml` - saved queries (committed)
- `state/` - per-user workspace state (git-ignored)

## App runtime

The desktop app is **pure Rust** and lives in `apps/desktop`.

- UI: `gpui` + `gpui-component`
- Async runtime bridging: `gpui_tokio`
- Data engines: SQLx-backed SQL engines and MongoDB driver
- Windowing: native multi-window behavior through GPUI app/window primitives

```mermaid
flowchart LR
  subgraph nativeClient [DesktopApp]
    Workspace[WorkspaceShell]
    Editors[QueryEditors]
    Inspectors[SchemaInspectors]
    DataViewers[DataViewers]
  end

  subgraph services [AppServices]
    Config[ProjectConfig]
    ConnRegistry[ConnectionRegistry]
    EngineLayer[EngineModules]
    Watchers[ProjectWatchers]
  end

  Workspace --> ConnRegistry
  Editors --> EngineLayer
  Inspectors --> EngineLayer
  DataViewers --> EngineLayer
  Config --> ConnRegistry
  Watchers --> Workspace
```

## Repository layout

```text
based/
├── apps/
│   └── desktop/
├── docs/
├── .based/
├── Cargo.toml
└── mise.toml
```

## Desktop module layers

`apps/desktop/src/` is organized by responsibility (single binary; no extra crates until a second target needs shared code):

| Layer | Path | Role |
|-------|------|------|
| Project | `project/` | `.based/` config, queries, file watching — no GPUI |
| Connection | `connection/` | Registry, open/close lifecycle — minimal engine tagging |
| Engines | `postgres/`, `sqlite/`, `mongodb/` | Drivers, schema, execution, engine panels |
| Workspace | `workspace/` | `Workspace` entity, tabs, dock, `tab_dispatch`, `connection_tree/` |
| Chrome | `workspace/chrome/` | Title bar, status bar, shell layout, GPUI overlay stack |
| Widgets | `widgets/` | Reusable panel UI (tables, editors, filters) |
| App | `app/` | Globals, actions, prefs, keybindings |

**Chrome dependency rule:** `workspace/chrome/` may use `widgets/`, `app/`, `bindings/`, and `connection/` types. It must not import engine modules, `tab_dispatch`, or `connection_tree/`.

**Orchestration vs domain shell:** Tab open/focus and dock wiring stay in `workspace/`. `connection_tree/`, `inspector`, and `welcome` are workspace “domain shell” (navigation), not generic chrome. `command_palette` stays at workspace root (opens tabs via workspace events).

## Day-to-day invariants

1. The `desktop` package is the only desktop runtime target.
2. CI/release workflows use Cargo-only pipelines.
3. New backend or engine work is implemented in Rust modules under `apps/desktop/src`.
4. Shared code extraction should move to `crates/*` only when a module is reused by more than one binary/library target.
