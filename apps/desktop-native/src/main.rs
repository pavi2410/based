// based — native GPUI desktop client
//
// Module layout mirrors the plan's repo structure.  Stubs for every module
// are declared here so `cargo check` validates the tree even before each
// phase fills in real implementations.

mod app;
mod connection;
mod db;
mod mongodb;
mod postgres;
mod project;
mod settings_window;
mod sqlite;
mod widgets;
mod workspace;

use gpui::prelude::*;
use gpui::*;
use gpui_component::{ActiveTheme, Root, Theme, ThemeMode, TitleBar};

use workspace::Workspace;

// ── Entry point ──────────────────────────────────────────────────────────────

fn main() {
    env_logger::Builder::from_env(env_logger::Env::default().default_filter_or("warn"))
    .format_timestamp_millis()
    .init();

    gpui_platform::application()
        .with_assets(gpui_component_assets::Assets)
        .run(move |cx| {
            gpui_component::init(cx);
            db::init(cx);

            cx.spawn(async move |cx| {
                cx.open_window(
                    WindowOptions {
                        window_bounds: Some(WindowBounds::Windowed(Bounds {
                            origin: point(px(100.0), px(100.0)),
                            size: size(px(1280.0), px(800.0)),
                        })),
                        titlebar: Some(TitleBar::title_bar_options()),
                        ..Default::default()
                    },
                    |window, cx| {
                        Theme::change(ThemeMode::Dark, Some(window), cx);
                        let workspace = cx.new(|cx| Workspace::new(window, cx));
                        cx.new(|cx| Root::new(workspace, window, cx).bg(cx.theme().background))
                    },
                )
                .expect("Failed to open main window");
            })
            .detach();
        });
}
