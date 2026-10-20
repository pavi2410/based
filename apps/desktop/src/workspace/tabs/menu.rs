//! Right-click menu and hover info for tab strip titles.

use gpui_kit::component::menu::{PopupMenu, PopupMenuItem};
use gpui_kit::{App, Context, EntityId, Window};

use super::label::is_panel_pinned;
use super::open::WorkspaceRef;
use crate::workspace::Workspace;

type WsAction = dyn Fn(&mut Workspace, &mut Window, &mut Context<Workspace>) + 'static;

fn item(
    label: &'static str,
    disabled: bool,
    f: impl Fn(&mut Workspace, &mut Window, &mut Context<Workspace>) + 'static,
) -> PopupMenuItem {
    let f: Box<WsAction> = Box::new(f);
    PopupMenuItem::new(label)
        .disabled(disabled)
        .on_click(move |_, window, app| {
            let Some(ws) = app.try_global::<WorkspaceRef>().map(|w| w.0.clone()) else {
                return;
            };
            ws.update(app, |ws, cx| f(ws, window, cx));
        })
}

/// Close / pin commands for one tab; mirrors the ⋮ menu's tab and bulk-close sections.
pub(crate) fn tab_context_menu(menu: PopupMenu, panel_id: EntityId, cx: &App) -> PopupMenu {
    let pinned = is_panel_pinned(panel_id, cx);
    menu.item(item("Close Tab", pinned, move |ws, w, cx| {
        ws.close_center_panel(panel_id, w, cx)
    }))
    .item(item(
        if pinned { "Unpin Tab" } else { "Pin Tab" },
        false,
        move |ws, _, cx| ws.toggle_pin_tab(panel_id, cx),
    ))
    .separator()
    .item(item("Close Others", false, move |ws, w, cx| {
        ws.close_other_tabs(panel_id, w, cx)
    }))
    .item(item("Close Tabs to the Left", false, move |ws, w, cx| {
        ws.close_tabs_to_left(panel_id, w, cx)
    }))
    .item(item("Close Tabs to the Right", false, move |ws, w, cx| {
        ws.close_tabs_to_right(panel_id, w, cx)
    }))
    .item(item("Close All", false, |ws, w, cx| {
        ws.close_all_tabs(w, cx)
    }))
}
