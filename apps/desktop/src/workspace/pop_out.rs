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

use super::tab_open::WorkspaceRef;
use crate::bindings::CloseTab;

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
            cx.quit();
        } else {
            PopOutManager::update_global(cx, |m, _| {
                m.windows.retain(|_, h| h.window_id() != closed_id);
            });
        }
    }
}

/// Append "Open in new window" to the tab ⋮ menu (merged before zoom/close by gpui-component).
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
    // Do not read this panel's Entity or call `can_close_center_panel` here — `dropdown_menu`
    // runs inside `Entity::update` and re-reading the panel panics.
    let close_disabled = !panel.closable(cx)
        || cx
            .try_global::<WorkspaceRef>()
            .is_none_or(|ws| ws.0.read(cx).center_tab_count(cx) <= 1);

    menu.separator()
        .item(
            PopupMenuItem::new("Close tab")
                .action(CloseTab.boxed_clone())
                .disabled(close_disabled)
                .on_click(move |_ev, window, app| {
                    let Some(workspace) = app.try_global::<WorkspaceRef>().map(|ws| ws.0.clone())
                    else {
                        return;
                    };
                    workspace.update(app, |ws, cx| {
                        ws.close_center_panel(panel_id, window, cx);
                    });
                }),
        )
        .item(
            PopupMenuItem::new("Open in new window").on_click(move |_ev, src_window, app| {
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

/// Short dock tab label and no zoom control in the tab-strip suffix.
#[macro_export]
macro_rules! based_panel_tab_chrome {
    () => {
        fn tab_name(&self, _: &gpui::App) -> Option<gpui::SharedString> {
            Some(self.tab_label.clone())
        }

        fn zoomable(&self, _: &gpui::App) -> Option<gpui_component::dock::PanelControl> {
            None
        }
    };
}
