// based — native GPUI desktop client
//
// Module layout mirrors the plan's repo structure.  Stubs for every module
// are declared here so `cargo check` validates the tree even before each
// phase fills in real implementations.

mod app;
mod bindings;
mod command_palette;
mod connection;
mod db;
mod mongodb;
mod postgres;
mod project;
mod query_store;
mod settings_window;
mod sqlite;
mod theme;
mod widgets;
mod workspace;

use gpui::prelude::*;
use gpui::*;
use gpui_component::{ActiveTheme, Root, TitleBar};

use workspace::{PopOutManager, SqlInject, TabOpenQueue, Workspace};

// ── Entry point ──────────────────────────────────────────────────────────────

fn main() {
    env_logger::Builder::from_env(env_logger::Env::default().default_filter_or("warn"))
        .format_timestamp_millis()
        .init();

    gpui_platform::application()
        .with_assets(gpui_component_assets::Assets)
        .run(move |cx| {
            gpui_component::init(cx);
            if let Err(err) = theme::install_based_theme(cx) {
                log::error!("failed to apply Based theme bundle: {err:#}");
            }
            bindings::init(cx);
            app::prefs::install(cx);

            db::init(cx);
            PopOutManager::init(cx);
            cx.set_global(TabOpenQueue::default());
            cx.set_global(SqlInject::default());

            let project_root = crate::project::find_project_root();
            query_store::init(project_root.clone(), cx);

            let vars = project_root
                .as_ref()
                .map(|root| {
                    crate::project::variables::load_variables(root).unwrap_or_else(|e| {
                        log::warn!("vars.toml load ({root:?}): {e:#}");
                        Default::default()
                    })
                })
                .unwrap_or_default();
            cx.set_global(crate::project::ProjectVars { vars });
            if let Some(root) = project_root.clone() {
                crate::project::install_reload_watcher(root, cx);
            }

            cx.on_window_closed(|cx, id| {
                PopOutManager::on_any_window_closed(cx, id);
            })
            .detach();

            cx.spawn(async move |cx| {
                let main = cx
                    .open_window(
                        WindowOptions {
                            window_bounds: Some(WindowBounds::Windowed(Bounds {
                                origin: point(px(100.0), px(100.0)),
                                size: size(px(1280.0), px(800.0)),
                            })),
                            titlebar: Some(TitleBar::title_bar_options()),
                            ..Default::default()
                        },
                        |window, cx| {
                            let workspace = cx.new(|cx| Workspace::new(window, cx));
                            cx.new(|cx| Root::new(workspace, window, cx))
                        },
                    )
                    .expect("Failed to open main window");
                cx.update_global(|manager: &mut PopOutManager, _| {
                    manager.main_window_id = Some(main.window_id());
                });
            })
            .detach();
        });
}
