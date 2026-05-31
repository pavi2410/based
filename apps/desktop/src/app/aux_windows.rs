//! Single-instance tracking for the App's auxiliary chrome windows (Settings, About).
//!
//! Mirrors the dedupe-and-focus pattern in [`crate::workspace::pop_out::PopOutManager`]
//! but keyed by [`AuxKind`] instead of a panel `EntityId`. Re-invoking
//! `open_settings` / `open_about` focuses the existing window rather than
//! opening a duplicate.
//!
//! Lifecycle:
//! - Each opener calls [`AuxWindows::focus_existing`] first; if it returns
//!   `true`, the new window is **not** opened.
//! - On successful `cx.open_window`, the opener calls [`AuxWindows::insert`].
//! - The global `cx.on_window_closed` listener in `main.rs` calls
//!   [`AuxWindows::on_window_closed`] to clear stale entries.
//! - When the main workspace window closes, [`PopOutManager::on_any_window_closed`]
//!   calls [`AuxWindows::close_all`] to cascade-close Settings + About before
//!   `cx.quit()`, matching the pop-out window behavior.

use std::collections::HashMap;

use gpui::{AnyWindowHandle, App, BorrowAppContext, Global, WindowId};

#[derive(Clone, Copy, Debug, Hash, PartialEq, Eq)]
pub enum AuxKind {
    Settings,
    About,
    Onboarding,
}

#[derive(Default)]
pub struct AuxWindows {
    windows: HashMap<AuxKind, AnyWindowHandle>,
}

impl Global for AuxWindows {}

impl AuxWindows {
    pub fn init(cx: &mut App) {
        cx.set_global(Self::default());
    }

    /// If a window for `kind` is already open, activate it and return `true`.
    pub fn focus_existing(kind: AuxKind, cx: &mut App) -> bool {
        let Some(handle) = cx.global::<Self>().windows.get(&kind).copied() else {
            return false;
        };
        let _ = handle.update(cx, |_, window, _| window.activate_window());
        true
    }

    pub fn insert(kind: AuxKind, handle: AnyWindowHandle, cx: &mut App) {
        cx.update_global(|m: &mut Self, _| {
            m.windows.insert(kind, handle);
        });
    }

    /// Drop any entries whose handle matches `closed`. Called for every window close.
    pub fn on_window_closed(closed: WindowId, cx: &mut App) {
        cx.update_global(|m: &mut Self, _| {
            m.windows.retain(|_, h| h.window_id() != closed);
        });
    }

    /// Close every tracked aux window. Called from `PopOutManager::on_any_window_closed`
    /// when the main workspace window closes, before `cx.quit()`.
    pub fn close_all(cx: &mut App) {
        let handles: Vec<AnyWindowHandle> = cx.global::<Self>().windows.values().copied().collect();
        cx.update_global(|m: &mut Self, _| m.windows.clear());
        for h in handles {
            let _ = h.update(cx, |_, window, _| window.remove_window());
        }
    }
}
