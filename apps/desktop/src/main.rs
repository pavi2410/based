// based — native GPUI desktop client
//
// Module layout mirrors the plan's repo structure.  Stubs for every module
// are declared here so `cargo check` validates the tree even before each
// phase fills in real implementations.

// gpui-component uses Arc<dyn Trait> patterns that aren't Send+Sync, and
// complex return types from builder chains — both are idiomatic in this ecosystem.
#![allow(clippy::arc_with_non_send_sync)]
#![allow(clippy::type_complexity)]

mod about_window;
mod app;
mod assets;
mod bindings;
mod command_palette;
mod connection;
mod db;
mod editor;
mod fonts;
mod mongodb;
mod onboarding_window;
mod postgres;
mod project;
mod query_store;
mod settings_window;
mod sqlite;
mod storage;
mod theme;
mod widgets;
mod workspace;

use connection::EngineRegistry;
use mongodb::MongoEngine;
use postgres::PostgresEngine;
use project::{
    ProjectContext, ProjectVars, find_project_root, install_reload_watcher,
    settings::apply_project_settings, variables::load_variables,
};
use sqlite::SqliteEngine;
use workspace::{PopOutManager, SqlInject, TabOpenQueue, WorkspaceNavQueue};

// ── Entry point ──────────────────────────────────────────────────────────────

fn main() {
    app::logging::init();

    gpui_platform::application()
        .with_assets(assets::ChainedAssets::new())
        .run(move |cx| {
            gpui_component::init(cx);
            if let Err(err) = theme::register_themes(cx) {
                log::error!("failed to register theme bundles: {err:#}");
            }
            fonts::register_bundled_fonts(cx);
            bindings::init(cx);
            command_palette::init(cx);
            app::shell::init(cx);
            app::prefs::install(cx);

            db::init(cx);

            // Engine registry — register new engines here; no other files need to change.
            {
                let mut registry = EngineRegistry::new();
                registry.register(PostgresEngine);
                registry.register(SqliteEngine);
                registry.register(MongoEngine);
                cx.set_global(registry);
            }

            cx.set_global(TabOpenQueue::default());
            cx.set_global(WorkspaceNavQueue::default());
            cx.set_global(SqlInject::default());
            app::updater::init(cx);
            if let Err(err) = storage::init(cx) {
                log::error!("failed to open metadata store: {err:#}");
            }
            PopOutManager::init(cx);
            app::aux_windows::AuxWindows::init(cx);
            app::launch::AppLaunch::init(cx);

            let project_root = find_project_root();
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
            if let Some(root) = project_root.clone() {
                install_reload_watcher(root, cx);
            }

            cx.on_window_closed(|cx, id| {
                PopOutManager::on_any_window_closed(cx, id);
                app::aux_windows::AuxWindows::on_window_closed(id, cx);
                if app::launch::AppLaunch::is_gate_window(id, cx) {
                    app::launch::AppLaunch::clear_gate(cx);
                }
            })
            .detach();

            app::launch::spawn_initial_window(cx);
        });
}
