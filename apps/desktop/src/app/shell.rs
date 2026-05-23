//! macOS shell integration: menu bar name and platform window titles.

use gpui::{App, Menu, MenuItem, SharedString, SystemMenuType, TitlebarOptions};
use gpui_component::TitleBar;

use super::quit;

pub const APP_NAME: &str = "Based";

gpui::actions!(app_shell, [QuitApp]);

/// Title bar options with a platform window title for Mission Control / ⌘`.
pub fn titled_titlebar(window_title: impl Into<SharedString>) -> TitlebarOptions {
    let mut options = TitleBar::title_bar_options();
    options.title = Some(window_title.into());
    options
}

pub fn init(cx: &mut App) {
    cx.activate(true);
    cx.on_action(|_: &QuitApp, cx| quit::request_app_quit(cx));
    cx.bind_keys([
        gpui::KeyBinding::new("cmd-q", QuitApp, None),
        gpui::KeyBinding::new("ctrl-q", QuitApp, None),
    ]);

    cx.set_menus([Menu::new(APP_NAME).items([
        MenuItem::os_submenu("Services", SystemMenuType::Services),
        MenuItem::separator(),
        MenuItem::action("Quit Based", QuitApp),
    ])]);
}
