use based_project::ProjectManifest;
use gpui::App;

/// Apply optional `[settings]` from `project.toml` into app preferences.
pub fn apply_project_settings(manifest: &ProjectManifest, cx: &mut App) {
    let Some(settings) = &manifest.settings else {
        return;
    };
    if let Some(timeout) = settings.query_timeout {
        crate::app::prefs::set_query_timeout_secs(timeout as u32, cx);
    }
}
