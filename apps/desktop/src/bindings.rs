//! Global key bindings for the native shell.

use gpui_kit::{App, KeyBinding};

gpui_kit::actions!([
    ToggleSidebarRail,
    CycleAppearance,
    ToggleCommandPalette,
    DismissCommandPalette,
    CloseTab,
    CloseAllTabs,
    CloseCleanTabs,
    CloseTabsLeft,
    CloseTabsRight,
    CloseOtherTabs,
    PinTab,
    GoBackTab,
    GoForwardTab,
    NewQuery,
    OpenSettings,
    OpenHome,
    SplitPaneTop,
    SplitPaneBottom,
    SplitPaneLeft,
    SplitPaneRight,
    ToggleInspectorPane,
    ToggleHistoryPane,
    ToggleSavedPane,
    ToggleChatPane,
    AskAi,
]);

/// `keys` behind the platform's primary modifier: ⌘ on macOS, Ctrl elsewhere.
/// Binding only one keeps Ctrl free for macOS text editing (ctrl-k kills a line,
/// ctrl-o opens one) and makes shortcut hints show ⌘.
#[cfg(target_os = "macos")]
macro_rules! primary {
    ($keys:literal) => {
        concat!("cmd-", $keys)
    };
}
#[cfg(not(target_os = "macos"))]
macro_rules! primary {
    ($keys:literal) => {
        concat!("ctrl-", $keys)
    };
}
pub(crate) use primary;

/// Key context set by the open command palette.
pub const PALETTE_CONTEXT: &str = "CommandPalette";

pub fn init(cx: &mut App) {
    cx.bind_keys([
        KeyBinding::new(primary!("\\"), ToggleSidebarRail, None),
        KeyBinding::new(primary!("alt-shift-t"), CycleAppearance, None),
        KeyBinding::new(primary!("k"), ToggleCommandPalette, None),
        KeyBinding::new("escape", DismissCommandPalette, Some(PALETTE_CONTEXT)),
        KeyBinding::new(primary!("w"), CloseTab, None),
        KeyBinding::new(primary!("alt-w"), CloseOtherTabs, None),
        KeyBinding::new(primary!("alt-shift-w"), CloseAllTabs, None),
        KeyBinding::new(primary!("alt-backspace"), CloseCleanTabs, None),
        KeyBinding::new(primary!("alt-left"), GoBackTab, None),
        KeyBinding::new(primary!("alt-right"), GoForwardTab, None),
        KeyBinding::new(primary!("alt-n"), NewQuery, None),
        KeyBinding::new(primary!("alt-i"), ToggleInspectorPane, None),
        KeyBinding::new(primary!("l"), ToggleChatPane, None),
        // ⌘⌥H is macOS "Hide Others".
        KeyBinding::new(primary!("y"), ToggleHistoryPane, None),
        KeyBinding::new(primary!("alt-s"), ToggleSavedPane, None),
        KeyBinding::new(primary!("i"), AskAi, None),
    ]);
}
