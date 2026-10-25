//! Settings: window lifecycle and preference persistence to the injected prefs file.

use based_desktop::app::paths::AppPaths;
use based_desktop::app::prefs::{self, AppearanceMode, NativePreferences};
use based_desktop::app::shell::open_settings;
use gpui_kit::test::TestWindowExt;
use gpui_kit::{AppContext, TestAppContext};

use crate::common::{init_app, no_project_env, settle};

fn setup(cx: &mut TestAppContext) {
    init_app(cx, &no_project_env().personal_root);
}

/// Reload what is on disk, as the next launch would.
fn on_disk(cx: &mut TestAppContext) -> NativePreferences {
    cx.update(|cx| NativePreferences::load_from(AppPaths::get(cx).prefs_path.clone()))
}

#[gpui_kit::test]
fn settings_window_opens_once(cx: &mut TestAppContext) {
    setup(cx);
    assert_eq!(cx.update(|cx| cx.windows().len()), 0);
    cx.update(open_settings);
    settle(cx);
    assert_eq!(cx.update(|cx| cx.windows().len()), 1);
    // Draw it: catches panics in the settings page builders.
    let win = cx.update(|cx| cx.windows()[0]);
    cx.update_window(win, |_, window, cx| window.render_frame(cx))
        .unwrap();
    cx.update(open_settings);
    settle(cx);
    assert_eq!(
        cx.update(|cx| cx.windows().len()),
        1,
        "second open reuses it"
    );
}

#[gpui_kit::test]
fn table_switch_persists(cx: &mut TestAppContext) {
    setup(cx);
    assert!(cx.update(|cx| prefs::table_prefs(cx).col_resizable));
    cx.update(|cx| prefs::set_table_col_resizable(false, cx));
    assert!(!cx.update(|cx| prefs::table_prefs(cx).col_resizable));
    assert!(!on_disk(cx).table_prefs.col_resizable);
    assert!(on_disk(cx).table_prefs.sortable, "other switches untouched");
}

#[gpui_kit::test]
fn page_size_is_clamped_and_saved(cx: &mut TestAppContext) {
    setup(cx);
    cx.update(|cx| prefs::set_page_size(u64::MAX, cx));
    let applied = cx.update(|cx| prefs::page_size(cx));
    assert!(applied < u64::MAX, "huge page size must be clamped");
    assert_eq!(on_disk(cx).page_size, applied);
}

#[gpui_kit::test]
fn query_timeout_is_clamped_and_saved(cx: &mut TestAppContext) {
    setup(cx);
    cx.update(|cx| prefs::set_query_timeout_secs(0, cx));
    let applied = cx.update(|cx| prefs::query_timeout_secs(cx));
    assert!(applied > 0, "zero timeout must be raised to the minimum");
    assert_eq!(on_disk(cx).query_timeout_secs, applied);
}

#[gpui_kit::test]
fn appearance_mode_persists(cx: &mut TestAppContext) {
    setup(cx);
    cx.update(|cx| prefs::apply_appearance(AppearanceMode::Light, None, cx));
    assert_eq!(
        cx.update(|cx| prefs::appearance_mode(cx)),
        AppearanceMode::Light
    );
    assert_eq!(on_disk(cx).appearance_mode, AppearanceMode::Light);
}

#[gpui_kit::test]
fn prefs_are_isolated_per_test_app(cx: &mut TestAppContext) {
    setup(cx);
    assert!(
        cx.update(|cx| prefs::table_prefs(cx).col_resizable),
        "fresh prefs file, not shared with other tests"
    );
}
