//! Auto-update preferences.

use super::*;

#[derive(Clone, Copy, Debug, Default, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum UpdateCheckInterval {
    #[default]
    Daily,
    Weekly,
    Monthly,
}

impl UpdateCheckInterval {
    pub const ALL: [Self; 3] = [Self::Daily, Self::Weekly, Self::Monthly];

    pub fn label(self) -> &'static str {
        match self {
            Self::Daily => "Daily",
            Self::Weekly => "Weekly",
            Self::Monthly => "Monthly",
        }
    }

    pub fn storage_key(self) -> &'static str {
        match self {
            Self::Daily => "daily",
            Self::Weekly => "weekly",
            Self::Monthly => "monthly",
        }
    }

    pub fn from_storage_key(key: &str) -> Option<Self> {
        match key {
            "daily" => Some(Self::Daily),
            "weekly" => Some(Self::Weekly),
            "monthly" => Some(Self::Monthly),
            _ => None,
        }
    }
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub struct UpdatePreferences {
    #[serde(default = "default_true")]
    pub check_at_startup: bool,
    #[serde(default = "default_true")]
    pub auto_check: bool,
    #[serde(default)]
    pub check_interval: UpdateCheckInterval,
    #[serde(default = "default_true")]
    pub auto_download: bool,
    #[serde(default = "default_true")]
    pub show_release_notes_after_update: bool,
    #[serde(default)]
    pub include_prereleases: bool,
}

impl Default for UpdatePreferences {
    fn default() -> Self {
        Self {
            check_at_startup: true,
            auto_check: true,
            check_interval: UpdateCheckInterval::Daily,
            auto_download: true,
            show_release_notes_after_update: true,
            include_prereleases: false,
        }
    }
}

pub fn update_prefs(cx: &App) -> UpdatePreferences {
    cx.global::<NativePreferences>().update.clone()
}

/// Dev builds cannot enable automatic update checks; settings UI reflects this.
pub fn update_check_settings_locked() -> bool {
    is_dev_build()
}

fn is_dev_build() -> bool {
    env!("CARGO_PKG_VERSION") == "0.0.0-dev"
}

pub(super) fn apply_dev_update_policy(update: &mut UpdatePreferences) {
    if update_check_settings_locked() {
        update.check_at_startup = false;
        update.auto_check = false;
    }
}

/// Whether manual "Check for Updates" actions should run (release builds only).
pub fn manual_update_checks_enabled() -> bool {
    !update_check_settings_locked()
}

fn update_update_prefs(update: impl FnOnce(&mut UpdatePreferences) -> bool, cx: &mut App) {
    let changed = cx.update_global(|p: &mut NativePreferences, _| {
        let changed = update(&mut p.update);
        if changed {
            p.save_best_effort();
        }
        changed
    });
    if changed {
        cx.refresh_windows();
    }
}

pub fn set_update_check_at_startup(enabled: bool, cx: &mut App) {
    if update_check_settings_locked() {
        return;
    }
    update_update_prefs(
        |u| {
            if u.check_at_startup == enabled {
                return false;
            }
            u.check_at_startup = enabled;
            true
        },
        cx,
    );
}

pub fn set_update_auto_check(enabled: bool, cx: &mut App) {
    if update_check_settings_locked() {
        return;
    }
    update_update_prefs(
        |u| {
            if u.auto_check == enabled {
                return false;
            }
            u.auto_check = enabled;
            true
        },
        cx,
    );
}

pub fn set_update_check_interval(interval: UpdateCheckInterval, cx: &mut App) {
    if update_check_settings_locked() {
        return;
    }
    update_update_prefs(
        |u| {
            if u.check_interval == interval {
                return false;
            }
            u.check_interval = interval;
            true
        },
        cx,
    );
}

pub fn set_update_auto_download(enabled: bool, cx: &mut App) {
    update_update_prefs(
        |u| {
            if u.auto_download == enabled {
                return false;
            }
            u.auto_download = enabled;
            true
        },
        cx,
    );
}

pub fn set_update_show_release_notes(enabled: bool, cx: &mut App) {
    update_update_prefs(
        |u| {
            if u.show_release_notes_after_update == enabled {
                return false;
            }
            u.show_release_notes_after_update = enabled;
            true
        },
        cx,
    );
}

pub fn set_update_include_prereleases(enabled: bool, cx: &mut App) {
    update_update_prefs(
        |u| {
            if u.include_prereleases == enabled {
                return false;
            }
            u.include_prereleases = enabled;
            true
        },
        cx,
    );
}
