//! macOS shell integration: menu bar items, app menu, and platform window titles.
//!
//! Owns the macOS app menubar (`Menu::new(APP_NAME)`) — File, Edit, View, an empty
//! Window menu (macOS injects Fill, Center, Minimize, Zoom, etc.), and Help.
//! Non-macOS platforms get app/help items via the topbar overflow menu in
//! [`crate::workspace::chrome::topbar`].

use gpui::{
    AnyWindowHandle, App, AppContext, Bounds, KeyBinding, Menu, MenuItem, OsAction, SharedString,
    SystemMenuType, TitlebarOptions, WindowBounds, WindowOptions, point, px, size,
};
use gpui_component::{
    Root, TitleBar,
    input::{Copy, Cut, Paste, Redo, SelectAll, Undo},
};

use super::aux_windows::{AuxKind, AuxWindows};
use super::quit;
use crate::about_window::AboutWindow;
use crate::bindings::{
    CloseAllTabs, CloseCleanTabs, CloseOtherTabs, CloseTab, CycleAppearance, GoBackTab,
    GoForwardTab, NewQuery, OpenOnboarding, OpenWelcome, SplitPaneBottom, SplitPaneLeft,
    SplitPaneRight, SplitPaneTop, ToggleCommandPalette, ToggleHistoryPane, ToggleInspectorPane,
    ToggleSavedPane, ToggleSidebarRail,
};
use crate::settings_window::SettingsWindow;
use crate::workspace::{WorkspaceRef, tab_open::enqueue_show_welcome};

pub const APP_NAME: &str = "Based";

gpui::actions!(
    app_shell,
    [
        QuitApp,
        AboutApp,
        OpenSettingsMenu,
        CheckForUpdates,
        OpenReleaseNotes,
        HideApp,
        HideOthers,
        ShowAll,
    ]
);

/// Title bar options with a platform window title for Mission Control / ⌘`.
pub fn titled_titlebar(window_title: impl Into<SharedString>) -> TitlebarOptions {
    let mut options = TitleBar::title_bar_options();
    options.title = Some(window_title.into());
    options
}

/// Title bar options for the Settings window.
///
/// On macOS, traffic lights are repositioned into the sidebar column (Zed-style)
/// via `traffic_light_position`; sidebar top padding is applied in
/// [`crate::settings_window::SettingsWindow`].
pub fn settings_titlebar() -> TitlebarOptions {
    let mut options = TitleBar::title_bar_options();
    options.title = Some("Based — Settings".into());
    #[cfg(target_os = "macos")]
    {
        options.traffic_light_position = Some(point(px(12.0), px(12.0)));
    }
    options
}

fn app_menu_items() -> Vec<MenuItem> {
    let mut items = vec![
        MenuItem::action("About Based", AboutApp),
        MenuItem::separator(),
        MenuItem::action("Settings...", OpenSettingsMenu),
        MenuItem::separator(),
    ];
    if crate::app::prefs::manual_update_checks_enabled() {
        items.push(MenuItem::action("Check for Updates…", CheckForUpdates));
        items.push(MenuItem::separator());
    }
    items.push(MenuItem::os_submenu("Services", SystemMenuType::Services));
    items.push(MenuItem::separator());
    #[cfg(target_os = "macos")]
    {
        items.push(MenuItem::action("Hide Based", HideApp));
        items.push(MenuItem::action("Hide Others", HideOthers));
        items.push(MenuItem::action("Show All", ShowAll));
        items.push(MenuItem::separator());
    }
    items.push(MenuItem::action("Quit Based", QuitApp));
    items
}

fn file_menu_items() -> Vec<MenuItem> {
    vec![
        MenuItem::action("New Query", NewQuery),
        MenuItem::separator(),
        MenuItem::action("Close Tab", CloseTab),
        MenuItem::action("Close Others", CloseOtherTabs),
        MenuItem::action("Close All", CloseAllTabs),
        MenuItem::action("Close Clean", CloseCleanTabs),
    ]
}

fn edit_menu_items() -> Vec<MenuItem> {
    vec![
        MenuItem::os_action("Undo", Undo, OsAction::Undo),
        MenuItem::os_action("Redo", Redo, OsAction::Redo),
        MenuItem::separator(),
        MenuItem::os_action("Cut", Cut, OsAction::Cut),
        MenuItem::os_action("Copy", Copy, OsAction::Copy),
        MenuItem::os_action("Paste", Paste, OsAction::Paste),
        MenuItem::separator(),
        MenuItem::os_action("Select All", SelectAll, OsAction::SelectAll),
    ]
}

fn view_menu_items() -> Vec<MenuItem> {
    vec![
        MenuItem::action("Command Palette…", ToggleCommandPalette),
        MenuItem::separator(),
        MenuItem::action("Toggle Sidebar", ToggleSidebarRail),
        MenuItem::action("Inspector", ToggleInspectorPane),
        MenuItem::action("History", ToggleHistoryPane),
        MenuItem::action("Saved Queries", ToggleSavedPane),
        MenuItem::separator(),
        MenuItem::action("Cycle Appearance", CycleAppearance),
        MenuItem::action("Back", GoBackTab),
        MenuItem::action("Forward", GoForwardTab),
        MenuItem::separator(),
        MenuItem::submenu(Menu::new("Split Pane").items([
            MenuItem::action("Split Left", SplitPaneLeft),
            MenuItem::action("Split Right", SplitPaneRight),
            MenuItem::action("Split Top", SplitPaneTop),
            MenuItem::action("Split Bottom", SplitPaneBottom),
        ])),
    ]
}

pub fn init(cx: &mut App) {
    cx.activate(true);
    cx.on_action(|_: &QuitApp, cx| quit::request_app_quit(cx));
    cx.on_action(|_: &AboutApp, cx| open_about(cx));
    cx.on_action(|_: &OpenSettingsMenu, cx| open_settings(cx));
    cx.on_action(|_: &OpenWelcome, cx| open_welcome(cx));
    cx.on_action(|_: &OpenOnboarding, cx| open_onboarding(cx));
    cx.on_action(|_: &CheckForUpdates, cx| crate::app::updater::check_now(cx));
    cx.on_action(|_: &OpenReleaseNotes, cx| {
        crate::app::updater::open_release_notes_for_current(cx)
    });
    #[cfg(target_os = "macos")]
    cx.on_action(|_: &HideApp, cx| cx.hide());
    cx.on_action(|_: &HideOthers, cx| cx.hide_other_apps());
    cx.on_action(|_: &ShowAll, cx| cx.unhide_other_apps());

    cx.bind_keys([
        KeyBinding::new("cmd-q", QuitApp, None),
        KeyBinding::new("ctrl-q", QuitApp, None),
        KeyBinding::new("cmd-,", OpenSettingsMenu, None),
    ]);

    cx.set_menus([
        Menu::new(APP_NAME).items(app_menu_items()),
        Menu::new("File").items(file_menu_items()),
        Menu::new("Edit").items(edit_menu_items()),
        Menu::new("View").items(view_menu_items()),
        // macOS injects Fill, Center, Minimize, Zoom, etc. via setWindowsMenu_.
        Menu::new("Window").items([]),
        Menu::new("Help").items([
            MenuItem::action("Welcome to Based", OpenWelcome),
            MenuItem::action("Onboarding...", OpenOnboarding),
            MenuItem::separator(),
            MenuItem::action("Release Notes", OpenReleaseNotes),
        ]),
    ]);
}

/// Focus or re-open the Welcome center tab (Help menu and topbar overflow).
pub fn open_welcome(cx: &mut App) {
    if cx.try_global::<WorkspaceRef>().is_none() {
        return;
    }
    enqueue_show_welcome(cx);
    cx.refresh_windows();
}

/// Open the onboarding review window (Help menu and topbar overflow).
pub fn open_onboarding(cx: &mut App) {
    if AuxWindows::focus_existing(AuxKind::Onboarding, cx) {
        return;
    }
    match crate::app::launch::open_onboarding_review(cx) {
        Ok(handle) => AuxWindows::insert(AuxKind::Onboarding, handle, cx),
        Err(err) => log::warn!("onboarding window: {err:#}"),
    }
}

/// Open the About window, or focus the existing one if already open.
pub fn open_about(cx: &mut App) {
    if AuxWindows::focus_existing(AuxKind::About, cx) {
        return;
    }
    let opened = cx.open_window(
        WindowOptions {
            window_bounds: Some(WindowBounds::centered(size(px(380.0), px(480.0)), cx)),
            titlebar: Some(titled_titlebar("About Based")),
            is_resizable: false,
            is_minimizable: false,
            ..Default::default()
        },
        |win, cx| {
            win.set_window_title("About Based");
            let about = cx.new(AboutWindow::new);
            cx.new(|cx| Root::new(about, win, cx))
        },
    );
    match opened {
        Ok(handle) => {
            let any: AnyWindowHandle = handle.into();
            AuxWindows::insert(AuxKind::About, any, cx);
        }
        Err(err) => log::warn!("about window: {err:#}"),
    }
}

/// Open the Settings window, or focus the existing one if already open.
///
/// Single source of truth used by the macOS app menu, ⌘, key binding,
/// the topbar overflow menu, and `Workspace::open_settings` (which is kept as a
/// thin shim so existing callers and the `OpenSettings` action listener still
/// work). Operating on `&mut App` directly avoids opening a window while
/// inside another window's update callback.
pub fn open_settings(cx: &mut App) {
    if AuxWindows::focus_existing(AuxKind::Settings, cx) {
        return;
    }
    let opened = cx.open_window(
        WindowOptions {
            window_bounds: Some(WindowBounds::Windowed(Bounds {
                origin: point(px(120.0), px(120.0)),
                size: size(px(800.0), px(600.0)),
            })),
            titlebar: Some(settings_titlebar()),
            ..Default::default()
        },
        |win, cx| {
            win.set_window_title("Based — Settings");
            let settings = cx.new(SettingsWindow::new);
            cx.new(|cx| Root::new(settings, win, cx))
        },
    );
    match opened {
        Ok(handle) => {
            let any: AnyWindowHandle = handle.into();
            AuxWindows::insert(AuxKind::Settings, any, cx);
        }
        Err(err) => log::warn!("settings window: {err:#}"),
    }
}
