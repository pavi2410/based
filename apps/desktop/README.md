# Based desktop app

Native GPUI database client (`apps/desktop` crate, binary `based`).

## Window launch sequence

Startup in `main.rs` initializes globals (prefs, storage, `PopOutManager`, `AuxWindows`), then calls `app::launch::spawn_initial_window`. All window opening logic lives in `app/launch.rs`.

1. App init — prefs, project context, metadata store, etc.
2. **Main workspace window** opens with the Home center tab and normal session restore.

Theme and keyboard shortcuts live in Settings. There is no first-run setup window.

### Auxiliary windows (any time after main exists)

| Window | Opener | Tracking |
|--------|--------|----------|
| Settings | App menu, ⌘,, topbar | `AuxWindows::Settings` |
| About | App menu, topbar | `AuxWindows::About` |

Pop-out editor tabs and aux windows are closed when the **main** workspace window closes (`PopOutManager::on_any_window_closed` → `AuxWindows::close_all` → `cx.quit()`).

## Related modules

- `app/launch.rs` — main workspace opening
- `app/aux_windows.rs` — single-instance aux window registry
- `workspace/` — main editor shell (dock, tabs, connection tree)
