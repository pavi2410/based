# Based desktop app

Native GPUI database client (`apps/desktop` crate, binary `based`).

## Window launch sequence

Startup in `main.rs` initializes globals (prefs, storage, `PopOutManager`, `AuxWindows`, `AppLaunch`), then calls `app::launch::spawn_initial_window`. All window opening logic lives in `app/launch.rs`.

### First run (`onboarding_completed == false`)

1. App init — prefs, project context, metadata store, etc. (no workspace window yet).
2. **Onboarding gate window** opens (`OnboardingWindow` in `FirstRunGate` mode).
   - Content: theme picker + keyboard shortcuts only.
   - **Finish Setup** or closing the window (traffic-light ×) both call `launch::complete_onboarding`:
     - Sets `onboarding_completed` in `native_preferences.toml`
     - Opens the main workspace window
3. **Main workspace window** opens with the Welcome center tab and normal session restore.

The gate window is tracked in `AppLaunch` (not `AuxWindows`). It is not the main window; closing it does not quit the app once completion runs.

### Returning users (`onboarding_completed == true`)

1. App init (same as above).
2. **Main workspace window** opens directly — Welcome tab + session restore. No onboarding gate.

### Auxiliary windows (any time after main exists)

| Window | Opener | Tracking |
|--------|--------|----------|
| Settings | App menu, ⌘,, topbar | `AuxWindows::Settings` |
| About | App menu, topbar | `AuxWindows::About` |
| Onboarding (review) | Help → Onboarding, command palette | `AuxWindows::Onboarding` |

Help → **Onboarding** reopens the same theme + shortcuts UI in **Review** mode. Closing that window only dismisses it; it does not change `onboarding_completed`.

Pop-out editor tabs and aux windows are closed when the **main** workspace window closes (`PopOutManager::on_any_window_closed` → `AuxWindows::close_all` → `cx.quit()`).

### Testing first-run again

Set `onboarding_completed = false` in native prefs (see `NativePreferences::prefs_path()` — typically `~/Library/Application Support/based/native_preferences.toml` on macOS) and restart the app.

## Related modules

- `onboarding_window/` — onboarding UI (gate + review)
- `app/launch.rs` — gate vs main workspace opening, `complete_onboarding`
- `app/aux_windows.rs` — single-instance aux window registry
- `workspace/` — main editor shell (dock, tabs, connection tree)
