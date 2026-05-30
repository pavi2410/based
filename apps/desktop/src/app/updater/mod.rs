mod check;
mod config;
mod install;
mod persist;
mod release_notes;
mod state;

pub use config::is_dev_build;
pub use install::open_releases_page;
pub use release_notes::fetch_release_body;
pub use state::{UpdateBarSnapshot, UpdatePhase};

use gpui::{App, AsyncApp, BorrowAppContext, Global, ParentElement, SharedString, Styled, Window};
use gpui_component::{ActiveTheme, WindowExt};

use crate::app::prefs::{self, UpdatePreferences};
use crate::app::quit;
use crate::connection::live_connection_count;
use crate::connection::registry::ConnectionRegistry;
use crate::workspace::notify;
use crate::workspace::tab_open::{WorkspaceRef, enqueue_open_release_notes};

use self::check::{check_packager_update, is_newer};
use self::persist::{
    UpdaterStateFile, should_run_periodic_check, startup_check_stale, updates_enabled,
};
use self::release_notes::fetch_latest_release;

/// Global update coordinator state (UI + scheduling).
#[derive(Clone, Default)]
pub struct UpdateCoordinator {
    pub phase: UpdatePhase,
    pub available_version: Option<String>,
    pub download_progress: u8,
    pub error: Option<String>,
    pub toast_shown_for_version: Option<String>,
}

impl Global for UpdateCoordinator {}

impl UpdateCoordinator {
    pub fn snapshot(&self) -> UpdateBarSnapshot {
        UpdateBarSnapshot {
            phase: self.phase,
            version: self
                .available_version
                .as_ref()
                .map(|v| SharedString::from(format!("v{v}"))),
            progress_percent: self.download_progress,
            error: self.error.as_ref().map(|e| SharedString::from(e.clone())),
        }
    }

    fn set_phase(&mut self, phase: UpdatePhase, cx: &mut App) {
        self.phase = phase;
        Self::notify_workspace(cx);
    }

    fn notify_workspace(cx: &mut App) {
        if let Some(ws) = cx.try_global::<WorkspaceRef>().map(|w| w.0.clone()) {
            ws.update(cx, |_, cx| cx.notify());
        }
        cx.refresh_windows();
    }

    fn prefs(cx: &App) -> UpdatePreferences {
        prefs::update_prefs(cx)
    }

    fn should_toast(&mut self, version: &str) -> bool {
        if self.toast_shown_for_version.as_deref() == Some(version) {
            return false;
        }
        self.toast_shown_for_version = Some(version.to_string());
        true
    }
}

pub fn coordinator_snapshot(cx: &App) -> UpdateBarSnapshot {
    cx.try_global::<UpdateCoordinator>()
        .map(|c| c.snapshot())
        .unwrap_or_default()
}

pub fn init(cx: &mut App) {
    if !updates_enabled() {
        return;
    }
    cx.set_global(UpdateCoordinator::default());

    let mut state = UpdaterStateFile::load();
    handle_pending_release_notes(&mut state, cx);

    cx.spawn(async move |cx| {
        tokio::time::sleep(std::time::Duration::from_secs(30)).await;
        cx.update(|app| {
            let prefs = UpdateCoordinator::prefs(app);
            if prefs.check_at_startup && startup_check_stale(&UpdaterStateFile::load()) {
                trigger_check(app, false);
            }
        });

        loop {
            let sleep_secs = cx.update(|app| {
                let prefs = UpdateCoordinator::prefs(app);
                persist::interval_secs(prefs.check_interval)
            });

            tokio::time::sleep(std::time::Duration::from_secs(sleep_secs)).await;

            let should = cx.update(|app| {
                let prefs = UpdateCoordinator::prefs(app);
                prefs.auto_check
                    && should_run_periodic_check(&UpdaterStateFile::load(), prefs.check_interval)
            });

            if should {
                cx.update(|app| trigger_check(app, false));
            }
        }
    })
    .detach();
}

fn handle_pending_release_notes(state: &mut UpdaterStateFile, cx: &mut App) {
    let Some(version) = state.take_pending_release_notes() else {
        return;
    };
    let show = UpdateCoordinator::prefs(cx).show_release_notes_after_update;
    if show {
        enqueue_open_release_notes(version, cx);
        cx.refresh_windows();
    } else {
        state.clear_pending_release_notes();
    }
}

/// Manual or scheduled update check.
pub fn check_now(cx: &mut App) {
    if !updates_enabled() {
        return;
    }
    trigger_check(cx, true);
}

pub fn dismiss(cx: &mut App) {
    cx.update_global(|coord: &mut UpdateCoordinator, app| {
        if let Some(v) = coord.available_version.clone() {
            let mut state = UpdaterStateFile::load();
            state.dismissed_version = Some(v);
            state.save_best_effort();
        }
        coord.available_version = None;
        coord.phase = UpdatePhase::Idle;
        UpdateCoordinator::notify_workspace(app);
    });
}

pub fn start_download(cx: &mut App) {
    if !updates_enabled() {
        return;
    }
    let version = cx.update_global(|coord: &mut UpdateCoordinator, _| {
        coord.phase = UpdatePhase::Downloading;
        coord.download_progress = 0;
        coord.available_version.clone()
    });
    let Some(version) = version else {
        return;
    };
    cx.spawn(async move |cx| {
        run_download_install(cx, version).await;
    })
    .detach();
    cx.update_global(|_: &mut UpdateCoordinator, app| UpdateCoordinator::notify_workspace(app));
}

pub fn install_and_restart(
    registry: &gpui::Entity<ConnectionRegistry>,
    window: &mut Window,
    cx: &mut App,
) {
    if !updates_enabled() {
        return;
    }
    let live = live_connection_count(registry, cx);
    if live == 0 {
        trigger_install(cx);
        return;
    }
    let description: SharedString = if live == 1 {
        "Restarting will disconnect 1 live connection to apply the update.".into()
    } else {
        format!("Restarting will disconnect {live} live connections to apply the update.").into()
    };
    let registry = registry.clone();
    window.open_alert_dialog(cx, move |alert, _window, cx| {
        use gpui_component::button::{Button, ButtonVariants};
        use gpui_component::dialog::{DialogAction, DialogClose, DialogFooter};
        let registry = registry.clone();
        let theme = cx.theme();
        let update_btn = Button::new("update-restart-confirm")
            .label("Install & Restart")
            .primary()
            .bg(theme.accent)
            .border_color(theme.accent);
        alert
            .title("Install update and restart?")
            .description(description.clone())
            .footer(
                DialogFooter::new()
                    .child(
                        DialogClose::new().child(
                            Button::new("update-restart-cancel")
                                .outline()
                                .label("Cancel"),
                        ),
                    )
                    .child(DialogAction::new().child(update_btn)),
            )
            .on_ok(move |_, _, cx| {
                quit::disconnect_all(&registry, cx);
                trigger_install(cx);
                true
            })
            .on_cancel(|_, _, _| true)
    });
}

pub fn open_release_notes_for_current(cx: &mut App) {
    let version = config::current_version_string().to_string();
    enqueue_open_release_notes(version, cx);
    cx.refresh_windows();
}

fn trigger_check(cx: &mut App, manual: bool) {
    cx.update_global(|coord: &mut UpdateCoordinator, app| {
        if coord.phase == UpdatePhase::Checking || coord.phase == UpdatePhase::Downloading {
            return;
        }
        coord.phase = UpdatePhase::Checking;
        coord.error = None;
        UpdateCoordinator::notify_workspace(app);
    });

    let manual = manual;
    cx.spawn(async move |cx| {
        run_check(cx, manual).await;
    })
    .detach();
}

fn trigger_install(cx: &mut App) {
    let version = cx.update_global(|coord: &mut UpdateCoordinator, _| {
        if coord.phase != UpdatePhase::Ready && coord.phase != UpdatePhase::Available {
            return None;
        }
        coord.phase = UpdatePhase::Downloading;
        coord.download_progress = 0;
        coord.available_version.clone()
    });
    let Some(version) = version else {
        return;
    };
    cx.spawn(async move |cx| {
        run_download_install(cx, version).await;
    })
    .detach();
    cx.update_global(|_: &mut UpdateCoordinator, app| UpdateCoordinator::notify_workspace(app));
}

async fn run_check(cx: &mut AsyncApp, manual: bool) {
    let prefs = cx.update(|app| UpdateCoordinator::prefs(app));

    let result = crate::db::run(cx, async move {
        let mut state = UpdaterStateFile::load();
        state.mark_checked_now();

        let current = config::current_version();

        // GitHub API for discovery / prerelease filtering.
        let github = fetch_latest_release(prefs.include_prereleases).await?;
        let github_newer = github.as_ref().filter(|g| is_newer(&g.version, &current));

        if !config::supports_in_app_install() {
            if let Some(g) = github_newer {
                return Ok(CheckOutcome::AvailableManual {
                    version: g.version_label.clone(),
                });
            }
            return Ok(CheckOutcome::UpToDate);
        }

        // Packager manifest for signed in-app update.
        let packager = tokio::task::spawn_blocking(check_packager_update)
            .await
            .context("join packager check")??;

        if let Some(_update) = packager {
            let version = _update.version.to_string();
            if !manual {
                let state = UpdaterStateFile::load();
                if state.dismissed_version.as_deref() == Some(version.as_str()) {
                    return Ok(CheckOutcome::Dismissed);
                }
            }
            return Ok(CheckOutcome::AvailableInApp { version });
        }

        if let Some(g) = github_newer {
            return Ok(CheckOutcome::AvailableManual {
                version: g.version_label.clone(),
            });
        }

        Ok(CheckOutcome::UpToDate)
    })
    .await;

    cx.update(|app| apply_check_outcome(app, result, &prefs));
}

enum CheckOutcome {
    UpToDate,
    Dismissed,
    AvailableManual { version: String },
    AvailableInApp { version: String },
}

fn apply_check_outcome(
    cx: &mut App,
    result: Result<CheckOutcome, anyhow::Error>,
    prefs: &UpdatePreferences,
) {
    let mut start_dl = false;
    cx.update_global(|coord: &mut UpdateCoordinator, app| match result {
        Ok(CheckOutcome::UpToDate) => {
            coord.phase = UpdatePhase::UpToDate;
            coord.available_version = None;
        }
        Ok(CheckOutcome::Dismissed) => {
            coord.phase = UpdatePhase::Idle;
        }
        Ok(CheckOutcome::AvailableManual { version }) => {
            coord.phase = UpdatePhase::Available;
            coord.available_version = Some(version.clone());
            if coord.should_toast(&version) {
                notify::push_update_available(app, &version);
            }
        }
        Ok(CheckOutcome::AvailableInApp { version }) => {
            coord.phase = UpdatePhase::Available;
            coord.available_version = Some(version.clone());
            if coord.should_toast(&version) {
                notify::push_update_available(app, &version);
            }
            if prefs.auto_download && config::supports_in_app_install() {
                start_dl = true;
            }
        }
        Err(err) => {
            log::warn!("update check failed: {err:#}");
            coord.phase = UpdatePhase::Failed;
            coord.error = Some(err.to_string());
        }
    });
    if start_dl {
        start_download(cx);
    } else {
        UpdateCoordinator::notify_workspace(cx);
    }
}

async fn run_download_install(cx: &mut AsyncApp, version: String) {
    let result = crate::db::run(cx, async {
        let update = tokio::task::spawn_blocking(check_packager_update)
            .await
            .context("join")??
            .context("no update available")?;
        if update.version != version {
            anyhow::bail!("update version mismatch");
        }
        tokio::task::spawn_blocking(move || {
            install::download_install_and_relaunch(update, &version)
        })
        .await
        .context("join install")?
    })
    .await;

    if result.is_err() {
        let msg = result.err().map(|e| e.to_string()).unwrap_or_default();
        cx.update(|app| {
            app.update_global(|coord: &mut UpdateCoordinator, a| {
                coord.phase = UpdatePhase::Failed;
                coord.error = Some(msg);
                UpdateCoordinator::notify_workspace(a);
            });
        });
    }
}

use anyhow::Context as _;
