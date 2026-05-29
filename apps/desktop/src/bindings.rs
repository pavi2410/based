//! Global key bindings for the native shell.

use gpui::{App, KeyBinding};

gpui::actions!([
    ToggleSidebarRail,
    CycleAppearance,
    ToggleCommandPalette,
    DismissCommandPalette,
    CloseTab,
    NewQuery,
    OpenSettings,
    OpenWelcome,
    ToggleInspectorPane,
    ToggleHistoryPane,
    ToggleSavedPane,
]);

pub fn init(cx: &mut App) {
    cx.bind_keys([
        KeyBinding::new("cmd-\\", ToggleSidebarRail, None),
        KeyBinding::new("ctrl-\\", ToggleSidebarRail, None),
        KeyBinding::new("cmd-alt-shift-t", CycleAppearance, None),
        KeyBinding::new("ctrl-alt-shift-t", CycleAppearance, None),
        KeyBinding::new("cmd-k", ToggleCommandPalette, None),
        KeyBinding::new("ctrl-k", ToggleCommandPalette, None),
        KeyBinding::new("escape", DismissCommandPalette, None),
        KeyBinding::new("cmd-w", CloseTab, None),
        KeyBinding::new("ctrl-w", CloseTab, None),
        KeyBinding::new("cmd-alt-i", ToggleInspectorPane, None),
        KeyBinding::new("ctrl-alt-i", ToggleInspectorPane, None),
        KeyBinding::new("cmd-alt-h", ToggleHistoryPane, None),
        KeyBinding::new("ctrl-alt-h", ToggleHistoryPane, None),
        KeyBinding::new("cmd-alt-s", ToggleSavedPane, None),
        KeyBinding::new("ctrl-alt-s", ToggleSavedPane, None),
    ]);
}
