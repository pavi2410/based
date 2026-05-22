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
    ]);
}
