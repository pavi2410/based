//! Pop a docked tab into a new OS window reusing the same panel [`Entity`], so UI state stays shared.
//!
//! - At most **one** pop-out window per panel (`EntityId`); choosing the menu again focuses the
//!   existing window. The docked tab **stays** — both views render the same entity (mirror), not a move.
//! - When the **main** workspace window closes, all pop-outs are closed so the process isn't left with
//!   orphan windows holding references into a torn-down workspace.

use std::collections::HashMap;

use gpui::prelude::*;
use gpui::*;
use gpui_component::{
    Root, Theme,
    dock::Panel,
    menu::{PopupMenu, PopupMenuItem},
};

use super::dock_utils::{center_panel_by_id, center_tab_items, center_tab_panel_count};
use super::tab_open::{DockAreaRef, TabManagerRef, WorkspaceRef};
use crate::bindings::{
    CloseAllTabs, CloseCleanTabs, CloseOtherTabs, CloseTab, CloseTabsLeft, CloseTabsRight, PinTab,
    SplitPaneBottom, SplitPaneLeft, SplitPaneRight, SplitPaneTop,
};

/// Human-readable OS window title for a popped-out panel.
pub trait PopOutWindowTitle: Panel {
    fn pop_out_window_title(&mut self, _window: &mut Window, _cx: &mut App) -> String {
        self.panel_name().to_string()
    }
}

/// Tracks the main window and child pop-out windows (for cascade-close and de-duplication).
pub struct PopOutManager {
    pub(crate) main_window_id: Option<WindowId>,
    pub(crate) windows: HashMap<EntityId, AnyWindowHandle>,
}

impl Global for PopOutManager {}

impl PopOutManager {
    pub fn init(cx: &mut App) {
        cx.set_global(Self {
            main_window_id: None,
            windows: HashMap::new(),
        });
    }

    pub fn is_pop_out_panel(entity_id: EntityId, cx: &App) -> bool {
        Self::global(cx).windows.contains_key(&entity_id)
    }

    pub fn on_any_window_closed(cx: &mut App, closed_id: WindowId) {
        let is_main = PopOutManager::global(cx).main_window_id == Some(closed_id);
        if is_main {
            PopOutManager::update_global(cx, |m, app| {
                let handles: Vec<AnyWindowHandle> = m.windows.values().copied().collect();
                m.windows.clear();
                m.main_window_id = None;
                for h in handles {
                    let _ = h.update(app, |_, window, _| window.remove_window());
                }
            });
            crate::app::aux_windows::AuxWindows::close_all(cx);
            cx.quit();
        } else {
            PopOutManager::update_global(cx, |m, _| {
                m.windows.retain(|_, h| h.window_id() != closed_id);
            });
        }
    }
}

/// Whether this panel type may be closed as a center **tab** (Welcome and connection dashboard are fixed).
pub(crate) fn panel_type_allows_tab_close(panel_name: &str) -> bool {
    !matches!(panel_name, "WelcomePanel" | "ConnectionDashboard")
}

fn center_tab_count(cx: &App) -> usize {
    let Some(dock_ref) = cx.try_global::<DockAreaRef>() else {
        return 0;
    };
    let dock = dock_ref.0.read(cx);
    center_tab_items(dock.center())
        .map(|items| items.len())
        .unwrap_or(0)
}

fn is_tab_pinned(panel_id: EntityId, cx: &App) -> bool {
    cx.try_global::<TabManagerRef>()
        .and_then(|tm| tm.0.read(cx).tab_for_panel_id(panel_id))
        .is_some_and(|t| t.pinned)
}

fn can_close_center_tab(panel_id: EntityId, panel_name: &str, cx: &App) -> bool {
    if center_tab_count(cx) <= 1 {
        return false;
    }
    panel_type_allows_tab_close(panel_name) && !is_tab_pinned(panel_id, cx)
}

fn can_close_center_pane(panel_id: EntityId, cx: &App) -> bool {
    let Some(dock_ref) = cx.try_global::<DockAreaRef>() else {
        return false;
    };
    let dock = dock_ref.0.read(cx);
    let center = dock.center();
    if center_tab_panel_count(center) <= 1 {
        return false;
    }
    center_panel_by_id(center, panel_id, cx).is_some()
}

/// Append tab commands and "Open in new window" to the tab ⋮ menu.
///
/// Pass the panel as `&self` — do not call `cx.entity().read(cx)` here: `dropdown_menu` runs inside
/// `Entity::update`, and reading the same entity would panic.
pub fn append_pop_out_to_panel_menu<T: Panel + PopOutWindowTitle + 'static>(
    menu: PopupMenu,
    panel: &T,
    cx: &mut Context<T>,
) -> PopupMenu {
    let weak = cx.entity().downgrade();
    let panel_id = cx.entity().entity_id();
    let panel_name = panel.panel_name();
    let pinned = is_tab_pinned(panel_id, cx);
    // Do not read this panel's Entity or Workspace here — `dropdown_menu` runs inside
    // `Entity::update` and re-reading those entities would panic.
    let close_disabled =
        !panel_type_allows_tab_close(panel_name) || pinned || center_tab_count(cx) <= 1;
    let close_pane_disabled = !can_close_center_pane(panel_id, cx);

    menu
        // ── This tab ──────────────────────────────────────────────────────────
        .item(
            PopupMenuItem::new("Close Tab")
                .action(CloseTab.boxed_clone())
                .disabled(close_disabled)
                .on_click({
                    let panel_id = panel_id;
                    move |_ev, window, app| {
                        let Some(workspace) =
                            app.try_global::<WorkspaceRef>().map(|ws| ws.0.clone())
                        else {
                            return;
                        };
                        workspace.update(app, |ws, cx| {
                            ws.close_center_panel(panel_id, window, cx);
                        });
                    }
                }),
        )
        .item(
            PopupMenuItem::new(if pinned { "Unpin Tab" } else { "Pin Tab" })
                .action(PinTab.boxed_clone())
                .on_click({
                    let panel_id = panel_id;
                    move |_ev, _window, app| {
                        let Some(workspace) =
                            app.try_global::<WorkspaceRef>().map(|ws| ws.0.clone())
                        else {
                            return;
                        };
                        workspace.update(app, |ws, cx| {
                            ws.toggle_pin_tab(panel_id, cx);
                        });
                    }
                }),
        )
        .separator()
        // ── Bulk close ──────────────────────────────────────────────────────
        .item(
            PopupMenuItem::new("Close Others")
                .action(CloseOtherTabs.boxed_clone())
                .on_click({
                    let panel_id = panel_id;
                    move |_ev, window, app| {
                        let Some(workspace) =
                            app.try_global::<WorkspaceRef>().map(|ws| ws.0.clone())
                        else {
                            return;
                        };
                        workspace.update(app, |ws, cx| {
                            ws.close_other_tabs(panel_id, window, cx);
                        });
                    }
                }),
        )
        .item(
            PopupMenuItem::new("Close Tabs to the Left")
                .action(CloseTabsLeft.boxed_clone())
                .on_click({
                    let panel_id = panel_id;
                    move |_ev, window, app| {
                        let Some(workspace) =
                            app.try_global::<WorkspaceRef>().map(|ws| ws.0.clone())
                        else {
                            return;
                        };
                        workspace.update(app, |ws, cx| {
                            ws.close_tabs_to_left(panel_id, window, cx);
                        });
                    }
                }),
        )
        .item(
            PopupMenuItem::new("Close Tabs to the Right")
                .action(CloseTabsRight.boxed_clone())
                .on_click({
                    let panel_id = panel_id;
                    move |_ev, window, app| {
                        let Some(workspace) =
                            app.try_global::<WorkspaceRef>().map(|ws| ws.0.clone())
                        else {
                            return;
                        };
                        workspace.update(app, |ws, cx| {
                            ws.close_tabs_to_right(panel_id, window, cx);
                        });
                    }
                }),
        )
        .item(
            PopupMenuItem::new("Close All")
                .action(CloseAllTabs.boxed_clone())
                .on_click(move |_ev, window, app| {
                    let Some(workspace) = app.try_global::<WorkspaceRef>().map(|ws| ws.0.clone())
                    else {
                        return;
                    };
                    workspace.update(app, |ws, cx| {
                        ws.close_all_tabs(window, cx);
                    });
                }),
        )
        .item(
            PopupMenuItem::new("Close Clean")
                .action(CloseCleanTabs.boxed_clone())
                .on_click(move |_ev, window, app| {
                    let Some(workspace) = app.try_global::<WorkspaceRef>().map(|ws| ws.0.clone())
                    else {
                        return;
                    };
                    workspace.update(app, |ws, cx| {
                        ws.close_clean_tabs(window, cx);
                    });
                }),
        )
        .separator()
        // ── Pane ──────────────────────────────────────────────────────────────
        .item(
            PopupMenuItem::new("Close Pane")
                .disabled(close_pane_disabled)
                .on_click({
                    let panel_id = panel_id;
                    move |_ev, window, app| {
                        let Some(workspace) =
                            app.try_global::<WorkspaceRef>().map(|ws| ws.0.clone())
                        else {
                            return;
                        };
                        workspace.update(app, |ws, cx| {
                            ws.close_center_pane(panel_id, window, cx);
                        });
                    }
                }),
        )
        .separator()
        // ── Split ─────────────────────────────────────────────────────────────
        .item(
            PopupMenuItem::new("Split Left")
                .action(SplitPaneLeft.boxed_clone())
                .on_click(move |_ev, window, app| {
                    let Some(workspace) = app.try_global::<WorkspaceRef>().map(|ws| ws.0.clone())
                    else {
                        return;
                    };
                    workspace.update(app, |ws, cx| {
                        ws.split_center_pane(gpui_component::Placement::Left, window, cx);
                    });
                }),
        )
        .item(
            PopupMenuItem::new("Split Right")
                .action(SplitPaneRight.boxed_clone())
                .on_click(move |_ev, window, app| {
                    let Some(workspace) = app.try_global::<WorkspaceRef>().map(|ws| ws.0.clone())
                    else {
                        return;
                    };
                    workspace.update(app, |ws, cx| {
                        ws.split_center_pane(gpui_component::Placement::Right, window, cx);
                    });
                }),
        )
        .item(
            PopupMenuItem::new("Split Top")
                .action(SplitPaneTop.boxed_clone())
                .on_click(move |_ev, window, app| {
                    let Some(workspace) = app.try_global::<WorkspaceRef>().map(|ws| ws.0.clone())
                    else {
                        return;
                    };
                    workspace.update(app, |ws, cx| {
                        ws.split_center_pane(gpui_component::Placement::Top, window, cx);
                    });
                }),
        )
        .item(
            PopupMenuItem::new("Split Bottom")
                .action(SplitPaneBottom.boxed_clone())
                .on_click(move |_ev, window, app| {
                    let Some(workspace) = app.try_global::<WorkspaceRef>().map(|ws| ws.0.clone())
                    else {
                        return;
                    };
                    workspace.update(app, |ws, cx| {
                        ws.split_center_pane(gpui_component::Placement::Bottom, window, cx);
                    });
                }),
        )
        .separator()
        // ── Window ────────────────────────────────────────────────────────────
        .item(
            PopupMenuItem::new("Open in New Window").on_click(move |_ev, src_window, app| {
                let Some(ent) = weak.upgrade() else {
                    return;
                };
                let panel_id = ent.entity_id();

                if let Some(existing) = PopOutManager::global(app).windows.get(&panel_id).copied() {
                    let _ = existing.update(app, |_, window, _| window.activate_window());
                    return;
                }

                let title_for_window = ent.update(app, |panel, cx| {
                    format!(
                        "{} — {}",
                        panel.pop_out_window_title(src_window, cx),
                        crate::app::shell::APP_NAME
                    )
                });
                let origin = {
                    let b = src_window.bounds();
                    b.origin + point(px(48.0), px(48.0))
                };
                let pop_bounds = Bounds {
                    origin,
                    size: size(px(960.0), px(640.0)),
                };
                match app.open_window(
                    WindowOptions {
                        window_bounds: Some(WindowBounds::Windowed(pop_bounds)),
                        titlebar: Some(crate::app::shell::titled_titlebar(
                            title_for_window.clone(),
                        )),
                        ..Default::default()
                    },
                    move |win, cx| {
                        Theme::change(Theme::global(cx).mode, Some(win), cx);
                        win.set_window_title(&title_for_window);
                        let v: AnyView = ent.clone().into();
                        cx.new(|cx| Root::new(v, win, cx))
                    },
                ) {
                    Ok(handle) => {
                        let any: AnyWindowHandle = handle.into();
                        PopOutManager::update_global(app, |m, _| {
                            m.windows.insert(panel_id, any);
                        });
                    }
                    Err(err) => log::warn!("pop-out window: {err:#}"),
                }
            }),
        )
}

#[macro_export]
macro_rules! based_panel_dropdown {
    ($menu:expr, $panel:expr, $cx:expr) => {
        $crate::workspace::pop_out::append_pop_out_to_panel_menu($menu, $panel, $cx)
    };
}

/// Dock tab label (via `title`) and no zoom control in the tab-strip suffix.
#[macro_export]
macro_rules! based_panel_tab_chrome {
    () => {
        fn tab_name(&self, _: &gpui::App) -> Option<gpui::SharedString> {
            None
        }

        fn title(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
            $crate::workspace::tab_label::render_strip_tab(
                self.tab_label.clone(),
                false,
                cx.entity().entity_id(),
                cx,
            )
        }

        fn closable(&self, _: &gpui::App) -> bool {
            false
        }

        fn zoomable(&self, _: &gpui::App) -> Option<gpui_component::dock::PanelControl> {
            None
        }
    };
    (dirty) => {
        fn tab_name(&self, _: &gpui::App) -> Option<gpui::SharedString> {
            None
        }

        fn title(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
            $crate::workspace::tab_label::render_strip_tab(
                self.tab_label.clone(),
                self.dirty,
                cx.entity().entity_id(),
                cx,
            )
        }

        fn closable(&self, _: &gpui::App) -> bool {
            false
        }

        fn zoomable(&self, _: &gpui::App) -> Option<gpui_component::dock::PanelControl> {
            None
        }
    };
}
