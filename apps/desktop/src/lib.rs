//! Based — native GPUI desktop client (library target).
//!
//! The `based` binary is a thin wrapper around [`run`]; keeping the app in a
//! library lets integration tests under `tests/` drive real views.

// gpui-kit uses Arc<dyn Trait> patterns that aren't Send+Sync, and
// complex return types from builder chains — both are idiomatic in this ecosystem.
#![allow(clippy::arc_with_non_send_sync)]
#![allow(clippy::type_complexity)]

pub mod about_window;
pub mod ai;
pub mod app;
pub mod assets;
pub mod bindings;
pub mod command_palette;
pub mod connection;
pub mod db;
pub mod editor;
pub mod fonts;
pub mod mongodb;
pub mod postgres;
pub mod project;
pub mod query_store;
pub mod redis;
pub mod settings_window;
pub mod sqlite;
pub mod storage;
pub mod theme;
pub mod widgets;
pub mod workspace;

use app::paths::AppPaths;
use gpui_kit::App;
use project::{
    ProjectContext, ProjectVars, find_project_root, install_reload_watcher,
    settings::apply_project_settings, variables::load_variables,
};
use workspace::{Inbox, PopOutManager};

/// Launch the desktop app (blocks until quit).
pub fn run() {
    app::logging::init();

    gpui_kit::application()
        .with_assets(assets::ChainedAssets::default())
        .run(move |cx| {
            init_globals(cx);
            if let Err(err) = storage::init(cx) {
                log::error!("failed to open metadata store: {err:#}");
            }
            app::updater::init(cx);
            init_project(cx);

            cx.on_window_closed(|cx, id| {
                PopOutManager::on_any_window_closed(cx, id);
                app::aux_windows::AuxWindows::on_window_closed(id, cx);
            })
            .detach();

            app::launch::spawn_initial_window(cx);
        });
}

/// Side-effect-free app globals (themes, fonts, bindings, queues, runtime).
///
/// Everything here is safe to call from headless tests: no disk, network,
/// file watchers or windows.
///
/// Installs [`AppPaths::from_env`] unless the caller already set one.
pub fn init_globals(cx: &mut App) {
    if !cx.has_global::<AppPaths>() {
        cx.set_global(AppPaths::from_env());
    }
    gpui_kit::init(cx);
    if let Err(err) = theme::register_themes(cx) {
        log::error!("failed to register theme bundles: {err:#}");
    }
    fonts::register_bundled_fonts(cx);
    bindings::init(cx);
    app::shell::init(cx);
    app::prefs::install(cx);

    db::init(cx);
    ai::init(cx);

    cx.set_global(Inbox::default());
    // Empty until `init_project` loads the project's vars.
    cx.set_global(ProjectVars::default());
    PopOutManager::init(cx);
    app::aux_windows::AuxWindows::init(cx);
}

/// Discover and load the `.based/` project, then start the reload watcher.
fn init_project(cx: &mut App) {
    let project_root = find_project_root(&AppPaths::get(cx).clone());
    let project_context = project_root
        .as_ref()
        .and_then(|root| ProjectContext::load(root.clone()).ok());
    if let Some(ref ctx) = project_context {
        cx.set_global(ctx.clone());
        apply_project_settings(&ctx.snapshot.manifest, cx);
    }
    query_store::init(
        project_root.clone(),
        project_context.as_ref().map(|c| c.snapshot.clone()),
        cx,
    );

    let vars = project_root
        .as_ref()
        .map(|root| {
            load_variables(root).unwrap_or_else(|e| {
                log::warn!("vars.toml load ({root:?}): {e:#}");
                Default::default()
            })
        })
        .unwrap_or_default();
    cx.set_global(ProjectVars { vars });
    if let Some(root) = project_root {
        install_reload_watcher(root, cx);
    }
}
