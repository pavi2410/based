//! macOS shell integration: menu bar items, app menu, and platform window titles.
//!
//! Owns the macOS app menubar (`Menu::new(APP_NAME)`) — only fully rendered on
//! macOS, where it includes About / Settings / Services / Quit. Non-macOS
//! platforms get the same items via the topbar overflow menu in
//! [`crate::workspace::chrome::topbar`].

use gpui::{
    AnyWindowHandle, App, AppContext, Bounds, KeyBinding, Menu, MenuItem, SharedString,
    SystemMenuType, TitlebarOptions, WindowBounds, WindowOptions, point, px, size,
};
use gpui_component::{Root, TitleBar};

use super::aux_windows::{AuxKind, AuxWindows};
use super::quit;
use crate::about_window::AboutWindow;
use crate::bindings::{OpenOnboarding, OpenWelcome};
use crate::settings_window::SettingsWindow;
use crate::workspace::{
    WorkspaceRef, tab_open::enqueue_show_onboarding, tab_open::enqueue_show_welcome,
};

pub const APP_NAME: &str = "Based";

gpui::actions!(app_shell, [QuitApp, AboutApp, OpenSettingsMenu]);

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

pub fn init(cx: &mut App) {
    cx.activate(true);
    cx.on_action(|_: &QuitApp, cx| quit::request_app_quit(cx));
    cx.on_action(|_: &AboutApp, cx| open_about(cx));
    cx.on_action(|_: &OpenSettingsMenu, cx| open_settings(cx));
    cx.on_action(|_: &OpenWelcome, cx| open_welcome(cx));
    cx.on_action(|_: &OpenOnboarding, cx| open_onboarding(cx));

    cx.bind_keys([
        KeyBinding::new("cmd-q", QuitApp, None),
        KeyBinding::new("ctrl-q", QuitApp, None),
        KeyBinding::new("cmd-,", OpenSettingsMenu, None),
    ]);

    cx.set_menus([
        Menu::new(APP_NAME).items([
            MenuItem::action("About Based", AboutApp),
            MenuItem::separator(),
            MenuItem::action("Settings...", OpenSettingsMenu),
            MenuItem::separator(),
            MenuItem::os_submenu("Services", SystemMenuType::Services),
            MenuItem::separator(),
            MenuItem::action("Quit Based", QuitApp),
        ]),
        Menu::new("Help").items([
            MenuItem::action("Welcome to Based", OpenWelcome),
            MenuItem::action("Onboarding...", OpenOnboarding),
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

/// Focus or re-open the Onboarding center tab (Help menu and topbar overflow).
pub fn open_onboarding(cx: &mut App) {
    if cx.try_global::<WorkspaceRef>().is_none() {
        return;
    }
    enqueue_show_onboarding(cx);
    cx.refresh_windows();
}

/// Open the About window, or focus the existing one if already open.
pub fn open_about(cx: &mut App) {
    if AuxWindows::focus_existing(AuxKind::About, cx) {
        return;
    }
    let opened = cx.open_window(
        WindowOptions {
            window_bounds: Some(WindowBounds::Windowed(Bounds {
                origin: point(px(160.0), px(160.0)),
                size: size(px(480.0), px(620.0)),
            })),
            titlebar: Some(titled_titlebar("About Based")),
            is_resizable: false,
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
            let settings = cx.new(|cx| SettingsWindow::new(win, cx));
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
