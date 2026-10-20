use std::sync::Mutex;
use std::sync::atomic::{AtomicU8, AtomicUsize, Ordering};
use std::time::Duration;

use tokio::task::spawn_blocking;
use tokio::time::sleep;
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

use gpui_kit::component::{ActiveTheme, WindowExt};
use gpui_kit::{
    App, AsyncApp, BorrowAppContext, Global, ParentElement, SharedString, Styled, Window,
};

use crate::connection::live_connection_count;
use crate::connection::registry::ConnectionRegistry;
use crate::db::{run, run_infallible};
use crate::workspace::notify;
use crate::workspace::tabs::{WorkspaceRef, enqueue_open_release_notes};

use super::prefs::{self, UpdatePreferences};
use super::quit;

use self::check::{check_packager_update, is_newer};
use self::log::{debug as udebug, info as uinfo, warn as uwarn};
use self::persist::{UpdaterStateFile, should_run_periodic_check, startup_check_stale};
use self::release_notes::fetch_latest_release;

/// Download progress (0-100) written from the blocking download thread.
static DL_PROGRESS: AtomicU8 = AtomicU8::new(0);

/// Verified update bundle waiting for the user to approve the install.
static STAGED: Mutex<Option<(cargo_packager_updater::Update, Vec<u8>)>> = Mutex::new(None);

/// Global update coordinator state (UI + scheduling).
#[derive(Clone, Default)]
pub struct UpdateCoordinator {
    pub phase: UpdatePhase,
    pub available_version: Option<String>,
    pub download_progress: u8,
    pub error: Option<String>,
    pub toast_shown_for_version: Option<String>,
    /// The available version can be downloaded and installed in-app.
    pub can_install: bool,
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
            can_install: self.can_install,
            error: self.error.as_ref().map(|e| SharedString::from(e.clone())),
        }
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
        sleep_on_tokio(cx, Duration::from_secs(30)).await;
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
            sleep_on_tokio(cx, Duration::from_secs(sleep_secs)).await;

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
async fn sleep_on_tokio(cx: &mut AsyncApp, duration: Duration) {
    let _ = run_infallible(cx, async move {
        sleep(duration).await;
    })
    .await;
}

fn handle_pending_release_notes(state: &mut UpdaterStateFile, cx: &mut App) {
    let Some(version) = state.take_pending_release_notes() else {
        return;
    };
    // Dev/debug builds cannot install updates; leftover pending notes must not
    // open What's New on every `cargo run`.
    if prefs::update_check_settings_locked() {
        udebug(format!(
            "pending release notes: cleared v{version} (dev build, updates locked)"
        ));
        return;
    }
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

/// Don't offer this version again (a manual check still shows it).
pub fn skip_version(cx: &mut App) {
    cx.update_global(|coord: &mut UpdateCoordinator, app| {
        if let Some(v) = coord.available_version.clone() {
            uinfo(format!("skip_version: version={v}"));
            let mut state = UpdaterStateFile::load();
            state.dismissed_version = Some(v);
            state.save_best_effort();
        }
        coord.available_version = None;
        coord.phase = UpdatePhase::Idle;
        UpdateCoordinator::notify_workspace(app);
    });
}

/// User-approved download of the available update; stages it for a later install.
pub fn start_download(cx: &mut App) {
    let version = cx.update_global(|coord: &mut UpdateCoordinator, _| {
        if coord.phase != UpdatePhase::Available || !coord.can_install {
            return None;
        }
        coord.phase = UpdatePhase::Downloading;
        coord.download_progress = 0;
        coord.available_version.clone()
    });
    let Some(version) = version else {
        uwarn("start_download: nothing to download");
        return;
    };
    uinfo(format!("start_download: version={version}"));
    DL_PROGRESS.store(0, Ordering::Relaxed);
    cx.spawn(async move |cx| {
        run_download(cx, version).await;
    })
    .detach();
    cx.spawn(async move |cx| {
        // The download runs on a blocking thread; poll its progress for the status bar.
        loop {
            sleep_on_tokio(cx, Duration::from_millis(250)).await;
            let downloading = cx.update(|app| {
                app.update_global(|coord: &mut UpdateCoordinator, app| {
                    if coord.phase != UpdatePhase::Downloading {
                        return false;
                    }
                    coord.download_progress = DL_PROGRESS.load(Ordering::Relaxed);
                    UpdateCoordinator::notify_workspace(app);
                    true
                })
            });
            if !downloading {
                break;
            }
        }
    })
    .detach();
    cx.update_global(|_: &mut UpdateCoordinator, app| UpdateCoordinator::notify_workspace(app));
}

/// Ask for confirmation, then install the downloaded update and restart.
pub fn install_and_restart(
    registry: &gpui_kit::Entity<ConnectionRegistry>,
    window: &mut Window,
    cx: &mut App,
) {
    let (phase, version) = {
        let coord = cx.global::<UpdateCoordinator>();
        (coord.phase, coord.available_version.clone())
    };
    if phase != UpdatePhase::Ready {
        udebug("install_and_restart: skipped (nothing downloaded)");
        return;
    }
    let live = live_connection_count(registry, cx);
    uinfo(format!(
        "install_and_restart: prompting (live_connections={live})"
    ));
    let mut description = format!(
        "Based {} has been downloaded. Based will quit and relaunch to install it.",
        version.as_deref().unwrap_or("update")
    );
    match live {
        0 => {}
        1 => description.push_str(" 1 live connection will be disconnected."),
        n => description.push_str(&format!(" {n} live connections will be disconnected.")),
    }
    let description: SharedString = description.into();
    let registry = registry.clone();
    window.open_alert_dialog(cx, move |alert, _window, cx| {
        use gpui_kit::component::button::{Button, ButtonVariants};
        use gpui_kit::component::dialog::{DialogAction, DialogClose, DialogFooter};
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
                                .label("Not now"),
                        ),
                    )
                    .child(DialogAction::new().child(update_btn)),
            )
            .on_ok(move |_, _, cx| {
                uinfo("install_and_restart: confirmed");
                quit::disconnect_all(&registry, cx);
                install_staged(cx);
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

/// Changelog of the update being offered (not the running version).
pub fn open_release_notes_for_available(cx: &mut App) {
    let Some(version) = cx.global::<UpdateCoordinator>().available_version.clone() else {
        return;
    };
    enqueue_open_release_notes(version.trim_start_matches('v').to_string(), cx);
    cx.refresh_windows();
}

fn trigger_check(cx: &mut App, manual: bool) {
    if prefs::update_check_settings_locked() {
        return;
    }
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
    cx.spawn(async move |cx| {
        run_check(cx, manual).await;
    })
    .detach();
}

fn install_staged(cx: &mut App) {
    let Some((update, bytes)) = STAGED.lock().ok().and_then(|mut g| g.take()) else {
        uwarn("install_staged: no staged update");
        return;
    };
    let version = update.version.to_string();
    uinfo(format!("install_staged: version={version}"));
    cx.spawn(async move |cx| {
        let result = run(cx, async move {
            spawn_blocking(move || install::install_and_relaunch(update, bytes, &version))
                .await
                .context("join install")?
        })
        .await;
        if let Err(err) = result {
            uwarn(format!("install_staged: failed {err:#}"));
            fail(cx, err.to_string());
        }
    })
    .detach();
}

fn fail(cx: &mut AsyncApp, msg: String) {
    cx.update(|app| {
        app.update_global(|coord: &mut UpdateCoordinator, a| {
            coord.phase = UpdatePhase::Failed;
            coord.error = Some(msg);
            UpdateCoordinator::notify_workspace(a);
        });
    });
}

async fn run_check(cx: &mut AsyncApp, manual: bool) {
    let prefs = cx.update(|app| UpdateCoordinator::prefs(app));

    let result = run(cx, async move {
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

        let skipped = |version: &str| {
            !manual && UpdaterStateFile::load().dismissed_version.as_deref() == Some(version)
        };

        if config::supports_in_app_install() {
            // Packager manifest for signed in-app update.
            udebug(format!(
                "run_check: packager manifest {}",
                config::UPDATE_MANIFEST_URL
            ));
            let packager = spawn_blocking(check_packager_update)
                .await
                .context("join packager check")??;
            if let Some(update) = packager {
                let version = update.version.to_string();
                uinfo(format!("run_check: update available version={version}"));
                if skipped(&version) {
                    return Ok(CheckOutcome::Skipped);
                }
                return Ok(CheckOutcome::AvailableInApp { version });
            }
            udebug("run_check: packager manifest has no newer update");
        } else {
            udebug("run_check: in-app install unsupported on this install path");
        }

        if let Some(g) = github_newer {
            if skipped(&g.version_label) {
                return Ok(CheckOutcome::Skipped);
            }
            return Ok(CheckOutcome::AvailableManual {
                version: g.version_label.clone(),
            });
        }
        Ok(CheckOutcome::UpToDate)
    })
    .await;

    cx.update(|app| {
        apply_check_outcome(app, result);
        let ready = {
            let coord = app.global::<UpdateCoordinator>();
            coord.phase == UpdatePhase::Available && coord.can_install
        };
        if ready && prefs.auto_download && !manual {
            start_download(app);
        }
    });
}

enum CheckOutcome {
    UpToDate,
    Skipped,
    AvailableManual { version: String },
    AvailableInApp { version: String },
}

fn apply_check_outcome(cx: &mut App, result: Result<CheckOutcome, anyhow::Error>) {
    cx.update_global(|coord: &mut UpdateCoordinator, app| match result {
        Ok(CheckOutcome::UpToDate) => {
            uinfo("check outcome: up_to_date");
            coord.phase = UpdatePhase::UpToDate;
            coord.available_version = None;
        }
        Ok(CheckOutcome::Skipped) => {
            udebug("check outcome: skipped version");
            coord.phase = UpdatePhase::Idle;
        }
        Ok(CheckOutcome::AvailableManual { version }) => {
            uinfo(format!("check outcome: available_manual version={version}"));
            coord.phase = UpdatePhase::Available;
            coord.can_install = false;
            coord.available_version = Some(version.clone());
            if coord.should_toast(&version) {
                notify::push_update_available(app, &version);
            }
        }
        Ok(CheckOutcome::AvailableInApp { version }) => {
            uinfo(format!("check outcome: available_in_app version={version}"));
            coord.phase = UpdatePhase::Available;
            coord.can_install = true;
            coord.available_version = Some(version.clone());
            if coord.should_toast(&version) {
                notify::push_update_available(app, &version);
            }
        }
        Err(err) => {
            uwarn(format!("check outcome: failed {err:#}"));
            coord.phase = UpdatePhase::Failed;
            coord.error = Some(err.to_string());
        }
    });
    UpdateCoordinator::notify_workspace(cx);
}

async fn run_download(cx: &mut AsyncApp, version: String) {
    uinfo(format!("download: starting version={version}"));
    let result = run(cx, async move {
        let update = spawn_blocking(check_packager_update)
            .await
            .context("join")??
            .context("no update available")?;
        if update.version != version {
            anyhow::bail!("update version mismatch");
        }
        uinfo(format!("download: fetching url={}", update.download_url));
        spawn_blocking(move || {
            let seen = AtomicUsize::new(0);
            let bytes = update
                .download_extended(
                    |chunk, total| {
                        let done = seen.fetch_add(chunk, Ordering::Relaxed) + chunk;
                        if let Some(total) = total.filter(|t| *t > 0) {
                            let pct = (done as u64 * 100 / total).min(100) as u8;
                            DL_PROGRESS.store(pct, Ordering::Relaxed);
                        }
                    },
                    || {},
                )
                .context("download update")?;
            Ok::<_, anyhow::Error>((update, bytes))
        })
        .await
        .context("join download")?
    })
    .await;

    match result {
        Ok(staged) => {
            uinfo("download: complete, staged for install");
            if let Ok(mut slot) = STAGED.lock() {
                *slot = Some(staged);
            }
            cx.update(|app| {
                app.update_global(|coord: &mut UpdateCoordinator, a| {
                    coord.phase = UpdatePhase::Ready;
                    coord.download_progress = 100;
                    UpdateCoordinator::notify_workspace(a);
                });
            });
        }
        Err(err) => {
            uwarn(format!("download: failed {err:#}"));
            fail(cx, err.to_string());
        }
    }
}

use anyhow::Context as _;
