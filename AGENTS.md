# AGENTS.md

This file provides guidance to AI coding agents (Claude Code, Codex, etc.) when working with code in this repository.

## Commands

```bash
# Via mise task runner
mise run dev      # Run desktop app
mise run build    # Release build
mise run check    # Fast compile check (no linking)
mise run lint     # Clippy
mise run format   # Format all code
mise run test     # Run all tests
mise run clean    # Remove build artifacts

# Direct cargo equivalents
cargo run -p desktop          # GUI binary: based
cargo build -p desktop --release
cargo check -p desktop
cargo clippy --workspace --all-targets
cargo fmt --all
cargo test --workspace
```

Start local databases for development:
```bash
docker compose up -d          # PostgreSQL 18 → localhost:15432, MongoDB 8 → localhost:37017
```

## Architecture

**Based** is a native desktop database client written in Rust using GPUI (from Zed Industries). It supports PostgreSQL, MongoDB, SQLite, and Redis. The project model is a `.based/` folder (committed to git) containing connection configs and saved queries — there is no backend service.

### Workspace layout

```
apps/desktop/src/        # GPUI app (package `desktop`, binary `based`)
├── main.rs              # Entry: GPUI app/window setup
├── app/                 # Global state, actions, preferences
├── connection/          # Engine-agnostic connection registry & lifecycle
├── workspace/           # Workspace entity, tabs, dock, connection_tree
│   └── chrome/          # Title bar, status bar, shell layout, overlays
├── sqlite/              # SQLite engine
├── postgres/            # PostgreSQL engine
├── mongodb/             # MongoDB engine
├── project/             # .based/ I/O glue: discovery, file watching, variables (parsing lives in crates/based-project)
├── widgets/             # Reusable UI components
├── theme/               # Visual theming
├── settings_window/     # Preferences UI
└── db.rs                # Tokio ↔ GPUI async bridge
```

A headless CLI is planned but does not exist yet; don't reference `apps/cli` or `-p cli`.

UI-free logic lives in `crates/` (no GPUI dependency):

| Crate | Role |
|-------|------|
| `based-core` | Shared types, session persistence, connection error taxonomy |
| `based-project` | Parse/load/validate the `.based/` format (connections, queries, env, favorites) |
| `based-query` | Query history, saved queries, variables, SQL helpers |
| `based-workspace` | Workspace/collection/environment models, template resolution |
| `based-storage` | Local SQLite metadata store (WAL) + keychain secret boundary |
| `based-postgres` / `based-sqlite` / `based-mongo` / `based-redis` | Per-engine driver logic |
| `based-ssh` | SSH local-forward tunnel used in front of an engine |

See [ARCHITECTURE.md](ARCHITECTURE.md) for dependency rules between layers.

### Key patterns

- **Engine abstraction**: `AnyConnection` enum + `Connectable` trait unify all database engines. Each engine implements open/test/close; query execution is engine-specific.
- **Async bridging**: `db.rs` bridges Tokio futures to GPUI tasks. All blocking I/O must go through this bridge to keep the UI responsive.
- **Tab system**: `TabId` encodes (connection id, tab kind, payload) — tabs are the primary navigation unit in the workspace.
- **PopOutManager**: Tracks detached child windows and their lifecycle; consult before creating new window types.
- **Project format**: `.based/project.toml` holds the manifest; `.based/connections/` and `.based/queries/` hold per-file connections and queries; `.based/.env` holds secrets (git-ignored); `.based/state/` holds per-user workspace state.

### Tab strip (dock)

Center editor tabs use gpui-kit `DockArea` + `DockSkin` (`PanelStyle::TabBar`), not the standalone Tabs story component. Layout is a `DockLayout` / `PaneTree` in gpui-base; wrap panels with `panel_handle` / `PanelHandle::from_view` so tab titles and menus survive. Labels come from `Panel::title` (short plain text via [`tab_label_for_spec`](apps/desktop/src/workspace/tabs/label.rs)); panel chrome uses `based_panel_behavior!` + `based_panel_tab_chrome!` (no zoom control).

**Close tabs:** Based keeps `BasePanel::closable` false and closes via **Close tab** in the panel ⋯ menu and **⌘W / Ctrl+W** (`CloseTab` → `Workspace::close_active_center_tab`) so pin, dirty, and Home-respawn rules stay in-app. Closing the last tab respawns **Home** via `ensure_home_tab`. Home is closable like any other tab when others are open; pinned tabs stay non-closable.

Each unpinned tab draws its own × inside the tab title ([`render_strip_tab`](apps/desktop/src/workspace/tabs/label.rs)) and closes via `Workspace::close_center_panel`, not gpui-kit's built-in close button (`set_close_button_visible`), which would bypass the pin/Home rules. The overflow chevron menu still needs upstream `TabGroup` API changes in [gpui-kit](https://github.com/longbridge/gpui-kit)—no fork in this repo.

### Clippy overrides

The workspace allows `arc_with_non_send_sync` and `type_complexity` to match gpui-kit conventions. Don't suppress other lints without justification.

### Performance profiles

Dev builds apply `opt-level = 3` to GPUI and rendering crates (see workspace `Cargo.toml` profile section) so the UI stays fast during development.
