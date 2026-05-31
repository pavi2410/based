mod check;
mod config;
mod install;
mod log;
mod persist;
mod release_notes;
mod state;

pub use install::{open_releases_page, relaunch_app};
pub use release_notes::fetch_release_body;
pub use state::{UpdateBarSnapshot, UpdatePhase};

use gpui::{App, AsyncApp, BorrowAppContext, Global, ParentElement, SharedString, Styled, Window};
use gpui_component::{ActiveTheme, WindowExt};

use crate::app::prefs::{self, UpdatePreferences};
use crate::app::quit;
use crate::connection::live_connection_count;
use crate::connection::registry::ConnectionRegistry;
use crate::workspace::notify;
use crate::workspace::tabs::{WorkspaceRef, enqueue_open_release_notes};

use self::check::{check_packager_update, is_newer};
use self::log::{debug as udebug, info as uinfo, warn as uwarn};
use self::persist::{UpdaterStateFile, should_run_periodic_check, startup_check_stale};
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
    uinfo(format!(
        "init current={} in_app={}",
        config::current_version_string(),
        config::supports_in_app_install()
    ));
    cx.set_global(UpdateCoordinator::default());

    let mut state = UpdaterStateFile::load();
    handle_pending_release_notes(&mut state, cx);

    cx.spawn(async move |cx| {
        udebug("startup check scheduled in 30s");
        sleep_on_tokio(cx, std::time::Duration::from_secs(30)).await;
        cx.update(|app| {
            let prefs = UpdateCoordinator::prefs(app);
            let stale = startup_check_stale(&UpdaterStateFile::load());
            if prefs.check_at_startup && stale {
                uinfo("startup check: running (stale or never checked)");
                trigger_check(app, false);
            } else {
                udebug(format!(
                    "startup check: skipped check_at_startup={} stale={}",
                    prefs.check_at_startup, stale
                ));
            }
        });

        loop {
            let sleep_secs = cx.update(|app| {
                let prefs = UpdateCoordinator::prefs(app);
                persist::interval_secs(prefs.check_interval)
            });

            udebug(format!("periodic loop: sleeping {sleep_secs}s"));
            sleep_on_tokio(cx, std::time::Duration::from_secs(sleep_secs)).await;

            let should = cx.update(|app| {
                let prefs = UpdateCoordinator::prefs(app);
                prefs.auto_check
                    && should_run_periodic_check(&UpdaterStateFile::load(), prefs.check_interval)
            });

            if should {
                uinfo("periodic check: running");
                cx.update(|app| trigger_check(app, false));
            } else {
                udebug("periodic check: skipped (auto_check off or interval not elapsed)");
            }
        }
    })
    .detach();
}

/// GPUI `cx.spawn` tasks are not on Tokio; delays must go through `db::run_infallible`.
async fn sleep_on_tokio(cx: &mut AsyncApp, duration: std::time::Duration) {
    let _ = crate::db::run_infallible(cx, async move {
        tokio::time::sleep(duration).await;
    })
    .await;
}

fn handle_pending_release_notes(state: &mut UpdaterStateFile, cx: &mut App) {
    let Some(version) = state.take_pending_release_notes() else {
        return;
    };
    let show = UpdateCoordinator::prefs(cx).show_release_notes_after_update;
    if show {
        uinfo(format!("pending release notes: opening tab for v{version}"));
        enqueue_open_release_notes(version, cx);
        cx.refresh_windows();
    } else {
        udebug(format!(
            "pending release notes: cleared v{version} (pref disabled)"
        ));
        state.clear_pending_release_notes();
    }
}

/// Manual or scheduled update check.
pub fn check_now(cx: &mut App) {
    if !prefs::manual_update_checks_enabled() {
        udebug("check_now: skipped (update checks locked)");
        return;
    }
    uinfo("check_now: manual check");
    trigger_check(cx, true);
}

pub fn dismiss(cx: &mut App) {
    cx.update_global(|coord: &mut UpdateCoordinator, app| {
        if let Some(v) = coord.available_version.clone() {
            uinfo(format!("dismiss: version={v}"));
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
    let version = cx.update_global(|coord: &mut UpdateCoordinator, _| {
        coord.phase = UpdatePhase::Downloading;
        coord.download_progress = 0;
        coord.available_version.clone()
    });
    let Some(version) = version else {
        uwarn("start_download: no available version");
        return;
    };
    uinfo(format!("start_download: version={version}"));
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
    let live = live_connection_count(registry, cx);
    if live == 0 {
        uinfo("install_and_restart: no live connections, proceeding");
        trigger_install(cx);
        return;
    }
    uinfo(format!(
        "install_and_restart: prompting (live_connections={live})"
    ));
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
                uinfo("install_and_restart: confirmed after connection gate");
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
    let skipped = cx.update_global(|coord: &mut UpdateCoordinator, app| {
        if coord.phase == UpdatePhase::Checking || coord.phase == UpdatePhase::Downloading {
            udebug(format!(
                "trigger_check: skipped phase={:?} manual={manual}",
                coord.phase
            ));
            return true;
        }
        coord.phase = UpdatePhase::Checking;
        coord.error = None;
        UpdateCoordinator::notify_workspace(app);
        false
    });
    if skipped {
        return;
    }

    uinfo(format!(
        "trigger_check: manual={manual} current={}",
        config::current_version_string()
    ));
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
        udebug("trigger_install: skipped (phase not Available/Ready)");
        return;
    };
    uinfo(format!("trigger_install: version={version}"));
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
        if let Some(g) = &github_newer {
            udebug(format!(
                "run_check: github latest={} newer=true",
                g.version_label
            ));
        } else if github.is_some() {
            udebug("run_check: github latest not newer than current");
        } else {
            udebug("run_check: no github latest (filtered or unavailable)");
        }

        if !config::supports_in_app_install() {
            udebug("run_check: in-app install unsupported on this install path");
            if let Some(g) = github_newer {
                return Ok(CheckOutcome::AvailableManual {
                    version: g.version_label.clone(),
                });
            }
            return Ok(CheckOutcome::UpToDate);
        }

        // Packager manifest for signed in-app update.
        udebug(format!(
            "run_check: packager manifest {}",
            config::UPDATE_MANIFEST_URL
        ));
        let packager = tokio::task::spawn_blocking(check_packager_update)
            .await
            .context("join packager check")??;

        if let Some(_update) = packager {
            let version = _update.version.to_string();
            uinfo(format!(
                "run_check: packager update available version={version}"
            ));
            if !manual {
                let state = UpdaterStateFile::load();
                if state.dismissed_version.as_deref() == Some(version.as_str()) {
                    udebug(format!("run_check: dismissed version={version}"));
                    return Ok(CheckOutcome::Dismissed);
                }
            }
            return Ok(CheckOutcome::AvailableInApp { version });
        }

        udebug("run_check: packager manifest has no newer update");
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
            uinfo("check outcome: up_to_date");
            coord.phase = UpdatePhase::UpToDate;
            coord.available_version = None;
        }
        Ok(CheckOutcome::Dismissed) => {
            udebug("check outcome: dismissed");
            coord.phase = UpdatePhase::Idle;
        }
        Ok(CheckOutcome::AvailableManual { version }) => {
            uinfo(format!("check outcome: available_manual version={version}"));
            coord.phase = UpdatePhase::Available;
            coord.available_version = Some(version.clone());
            if coord.should_toast(&version) {
                notify::push_update_available(app, &version);
            }
        }
        Ok(CheckOutcome::AvailableInApp { version }) => {
            uinfo(format!(
                "check outcome: available_in_app version={version} auto_download={}",
                prefs.auto_download
            ));
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
            uwarn(format!("check outcome: failed {err:#}"));
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
    uinfo(format!("download_install: starting version={version}"));
    let result = crate::db::run(cx, async {
        let update = tokio::task::spawn_blocking(check_packager_update)
            .await
            .context("join")??
            .context("no update available")?;
        if update.version != version {
            anyhow::bail!("update version mismatch");
        }
        uinfo(format!(
            "download_install: fetching url={}",
            update.download_url
        ));
        tokio::task::spawn_blocking(move || {
            install::download_install_and_relaunch(update, &version)
        })
        .await
        .context("join install")?
    })
    .await;

    if let Err(err) = result {
        uwarn(format!("download_install: failed {err:#}"));
        let msg = err.to_string();
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
