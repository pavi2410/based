// based — native GPUI desktop client
//
// Module layout mirrors the plan's repo structure.  Stubs for every module
// are declared here so `cargo check` validates the tree even before each
// phase fills in real implementations.

mod about_window;
mod app;
mod assets;
mod bindings;
mod command_palette;
mod connection;
mod db;
mod fonts;
mod mongodb;
mod postgres;
mod project;
mod query_store;
mod settings_window;
mod sqlite;
mod storage;
mod theme;
mod widgets;
mod workspace;

use gpui::prelude::*;
use gpui::*;
use gpui_component::Root;

use workspace::{PopOutManager, SqlInject, TabOpenQueue, Workspace, WorkspaceNavQueue};

// ── Entry point ──────────────────────────────────────────────────────────────

fn main() {
    env_logger::Builder::from_env(
        env_logger::Env::default().default_filter_or("warn,based_quit=warn"),
    )
    .format_timestamp_millis()
    .init();

    gpui_platform::application()
        .with_assets(assets::ChainedAssets::new())
        .run(move |cx| {
            gpui_component::init(cx);
            if let Err(err) = theme::register_themes(cx) {
                log::error!("failed to register theme bundles: {err:#}");
            }
            fonts::register_bundled_fonts(cx);
            bindings::init(cx);
            app::shell::init(cx);
            app::prefs::install(cx);

            db::init(cx);
            app::updater::init(cx);
            if let Err(err) = storage::init(cx) {
                log::error!("failed to open metadata store: {err:#}");
            }
            PopOutManager::init(cx);
            app::aux_windows::AuxWindows::init(cx);
            cx.set_global(TabOpenQueue::default());
            cx.set_global(WorkspaceNavQueue::default());
            cx.set_global(SqlInject::default());

            let project_root = crate::project::find_project_root();
            let project_context = project_root
                .as_ref()
                .and_then(|root| crate::project::ProjectContext::load(root.clone()).ok());
            if let Some(ref ctx) = project_context {
                cx.set_global(ctx.clone());
                crate::project::settings::apply_project_settings(&ctx.snapshot.manifest, cx);
            }
            query_store::init(
                project_root.clone(),
                project_context.as_ref().map(|c| c.snapshot.clone()),
                cx,
            );

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
                app::aux_windows::AuxWindows::on_window_closed(id, cx);
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
                            titlebar: Some(app::shell::titled_titlebar(app::shell::APP_NAME)),
                            ..Default::default()
                        },
                        |window, cx| {
                            window.set_window_title(app::shell::APP_NAME);
                            let workspace = cx.new(|cx| Workspace::new(window, cx));
                            cx.set_global(crate::workspace::WorkspaceRef(workspace.clone()));
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
