//! Application window launch: onboarding gate before the main workspace.

use gpui::{
    AnyWindowHandle, App, AppContext, BorrowAppContext, Bounds, Global, WindowBounds, WindowId,
    WindowOptions, point, px, size,
};
use gpui_component::Root;

use crate::app::prefs;
use crate::app::shell::{self, APP_NAME};
use crate::onboarding_window::{OnboardingMode, OnboardingWindow};
use crate::workspace::{PopOutManager, Workspace, WorkspaceRef};

/// Tracks the first-run onboarding gate window (not the Help-menu review window).
#[derive(Default)]
pub struct AppLaunch {
    pub gate_window_id: Option<WindowId>,
    pub gate_handle: Option<AnyWindowHandle>,
}

impl Global for AppLaunch {}

impl AppLaunch {
    pub fn init(cx: &mut App) {
        cx.set_global(Self::default());
    }

    pub fn is_gate_window(window_id: WindowId, cx: &App) -> bool {
        cx.global::<Self>().gate_window_id == Some(window_id)
    }

    pub fn register_gate(handle: AnyWindowHandle, cx: &mut App) {
        let window_id = handle.window_id();
        cx.update_global(|state: &mut Self, _| {
            state.gate_window_id = Some(window_id);
            state.gate_handle = Some(handle);
        });
    }

    pub fn clear_gate(cx: &mut App) {
        cx.update_global(|state: &mut Self, _| {
            state.gate_window_id = None;
            state.gate_handle = None;
        });
    }
}

/// Open the main workspace window (no-op if already open).
pub fn open_main_workspace(cx: &mut App) -> anyhow::Result<()> {
    if cx.try_global::<WorkspaceRef>().is_some() {
        return Ok(());
    }

    let opened = cx.open_window(
        WindowOptions {
            window_bounds: Some(WindowBounds::Windowed(Bounds {
                origin: point(px(100.0), px(100.0)),
                size: size(px(1280.0), px(800.0)),
            })),
            titlebar: Some(shell::titled_titlebar(APP_NAME)),
            ..Default::default()
        },
        |window, cx| {
            window.set_window_title(APP_NAME);
            let workspace = cx.new(|cx| Workspace::new(window, cx));
            cx.set_global(WorkspaceRef(workspace.clone()));
            cx.new(|cx| Root::new(workspace, window, cx))
        },
    )?;

    let any: AnyWindowHandle = opened.into();
    cx.update_global(|manager: &mut PopOutManager, _| {
        manager.main_window_id = Some(any.window_id());
    });
    Ok(())
}

/// First-run onboarding gate. Closing the window is equivalent to Finish Setup.
pub fn open_onboarding_gate(cx: &mut App) -> anyhow::Result<AnyWindowHandle> {
    let opened = cx.open_window(onboarding_window_options(cx), |window, cx| {
        window.set_window_title("Based — Setup");
        if !prefs::onboarding_completed(cx) {
            window.on_window_should_close(cx, |_window, cx| {
                complete_onboarding(cx);
                AppLaunch::clear_gate(cx);
                true
            });
        }
        let onboarding = cx.new(|cx| OnboardingWindow::new(OnboardingMode::FirstRunGate, cx));
        cx.new(|cx| Root::new(onboarding, window, cx))
    })?;

    let any: AnyWindowHandle = opened.into();
    AppLaunch::register_gate(any, cx);
    Ok(any)
}

/// Help-menu review window (theme + shortcuts). Close dismisses only this window.
pub fn open_onboarding_review(cx: &mut App) -> anyhow::Result<AnyWindowHandle> {
    let opened = cx.open_window(onboarding_window_options(cx), |window, cx| {
        window.set_window_title("Based — Onboarding");
        let onboarding = cx.new(|cx| OnboardingWindow::new(OnboardingMode::Review, cx));
        cx.new(|cx| Root::new(onboarding, window, cx))
    })?;
    Ok(opened.into())
}

fn onboarding_window_options(cx: &App) -> WindowOptions {
    WindowOptions {
        window_bounds: Some(WindowBounds::centered(size(px(680.0), px(560.0)), cx)),
        titlebar: Some(shell::titled_titlebar("Based — Setup")),
        ..Default::default()
    }
}

/// Mark onboarding complete and open the main workspace with Welcome.
pub fn complete_onboarding(cx: &mut App) {
    if !prefs::onboarding_completed(cx) {
        prefs::set_onboarding_completed(true, cx);
    }
    if let Err(err) = open_main_workspace(cx) {
        log::warn!("open main workspace after onboarding: {err:#}");
    }
}

/// Entry point after app init: onboarding gate or main workspace.
pub fn spawn_initial_window(cx: &mut App) {
    cx.spawn(async move |cx| {
        cx.update(|app| {
            if prefs::onboarding_completed(app) {
                if let Err(err) = open_main_workspace(app) {
                    log::error!("failed to open main workspace: {err:#}");
                }
            } else if let Err(err) = open_onboarding_gate(app) {
                log::error!("failed to open onboarding gate: {err:#}");
            }
        });
    })
    .detach();
}
