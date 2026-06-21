use based_project::ProjectManifest;
use gpui::App;

use crate::app::prefs::set_query_timeout_secs;

/// Convert project.toml `query_timeout` (milliseconds) to preference seconds.
pub fn query_timeout_ms_to_secs(ms: u64) -> u32 {
    ((ms / 1000).max(1)) as u32
}

/// Apply optional `[settings]` from `project.toml` into app preferences.
pub fn apply_project_settings(manifest: &ProjectManifest, cx: &mut App) {
    let Some(settings) = &manifest.settings else {
        return;
    };
    if let Some(timeout) = settings.query_timeout {
        set_query_timeout_secs(query_timeout_ms_to_secs(timeout), cx);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn converts_project_timeout_ms_to_secs() {
        assert_eq!(query_timeout_ms_to_secs(30_000), 30);
        assert_eq!(query_timeout_ms_to_secs(500), 1);
    }
}
