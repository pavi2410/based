# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

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
cargo run -p desktop
cargo build -p desktop --release
cargo check -p desktop
cargo clippy --workspace --all-targets
cargo fmt --all
cargo test --workspace
```

Start local databases for development:
```bash
docker compose up -d          # PostgreSQL 16 (5432), MongoDB 7 (27017)
```

## Architecture

**Based** is a native desktop database client written in Rust using GPUI (from Zed Industries). It supports PostgreSQL, MongoDB, and SQLite. The project model is a `.based/` folder (committed to git) containing connection configs and saved queries — there is no backend service.

### Workspace layout

```
apps/desktop/src/
├── main.rs              # Entry: GPUI app/window setup
├── app/                 # Global state, actions, preferences
├── connection/          # Engine-agnostic connection registry & lifecycle
├── workspace/           # Main UI shell: DockArea, panes, sidebar, status bar
├── sqlite/              # SQLite engine
├── postgres/            # PostgreSQL engine
├── mongodb/             # MongoDB engine
├── project/             # .based/ folder loading and file watching
├── widgets/             # Reusable UI components
├── theme/               # Visual theming
├── settings_window/     # Preferences UI
└── db.rs                # Tokio ↔ GPUI async bridge
```

### Key patterns

- **Engine abstraction**: `AnyConnection` enum + `Connectable` trait unify all database engines. Each engine implements open/test/close; query execution is engine-specific.
- **Async bridging**: `db.rs` uses `gpui_tokio` to bridge Tokio async tasks into GPUI's sync render cycle. All blocking I/O must go through this bridge to keep the UI responsive.
- **Tab system**: `TabId` encodes (connection id, tab kind, payload) — tabs are the primary navigation unit in the workspace.
- **PopOutManager**: Tracks detached child windows and their lifecycle; consult before creating new window types.
- **Project format**: `.based/config.toml` holds connection metadata; `.based/.env` holds secrets (git-ignored); `.based/state/` holds per-user workspace state.

### Tab strip (dock)

Center editor tabs use gpui-component `DockArea` + `PanelStyle::TabBar`, not the standalone Tabs story component. Labels come from `Panel::tab_name` (short plain text via [`tab_label_for_spec`](apps/desktop/src/workspace/tab_label.rs)); panel chrome uses `based_panel_tab_chrome!` (no zoom button in the tab suffix).

**Close tabs:** gpui-component hides “Close” on the center `TabPanel` when `stack_panel` is unset. Based works around this with **Close tab** in the panel ⋯ menu and **⌘W / Ctrl+W** (`CloseTab` → `Workspace::close_active_center_tab` → `DockArea::remove_panel`). Welcome and the connection dashboard tab are not closable; the last center tab cannot be closed.

Full Tabs-demo parity (per-tab ×, overflow chevron menu) needs upstream `TabPanel` API changes in [gpui-component](https://github.com/longbridge/gpui-component)—no fork in this repo.

### Clippy overrides

The workspace allows `arc_with_non_send_sync` and `type_complexity` to match gpui-component conventions. Don't suppress other lints without justification.

### Performance profiles

Dev builds apply `opt-level = 3` to GPUI and rendering crates (see workspace `Cargo.toml` profile section) so the UI stays fast during development.
