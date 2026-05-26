//! Right-edge activity rail: vertical icon strip controlling the side pane.

use gpui::{App, Entity, IntoElement, ParentElement, Styled, px};
use gpui_component::{
    ActiveTheme, Selectable as _, Sizable as _,
    button::{Button, ButtonVariants},
    v_flex,
};

use crate::bindings::{ToggleHistoryPane, ToggleInspectorPane, ToggleSavedPane};
use crate::workspace::Workspace;
use crate::workspace::chrome::side_pane::SidePane;

pub const ACTIVITY_RAIL_WIDTH: f32 = 40.0;

pub fn render_activity_rail(
    workspace: Entity<Workspace>,
    active: Option<SidePane>,
    cx: &App,
) -> impl IntoElement {
    let border = cx.theme().border;

    v_flex()
        .w(px(ACTIVITY_RAIL_WIDTH))
        .h_full()
        .flex_shrink_0()
        .items_center()
        .py_2()
        .gap_1()
        .border_l_1()
        .border_color(border)
        .bg(cx.theme().sidebar)
        .children(SidePane::ALL.map(|pane| rail_button(pane, active, workspace.clone())))
}

fn rail_button(
    pane: SidePane,
    active: Option<SidePane>,
    workspace: Entity<Workspace>,
) -> impl IntoElement {
    let is_selected = active == Some(pane);
    let id = match pane {
        SidePane::Inspector => "rail-inspector",
        SidePane::History => "rail-history",
        SidePane::Saved => "rail-saved",
    };

    let button = Button::new(id)
        .ghost()
        .small()
        .icon(pane.icon())
        .selected(is_selected)
        .on_click(move |_, _, cx| {
            let ws = workspace.clone();
            ws.update(cx, |w, cx| w.toggle_side_pane(pane, cx));
        });

    match pane {
        SidePane::Inspector => {
            button.tooltip_with_action(pane.tooltip(), &ToggleInspectorPane, None)
        }
        SidePane::History => button.tooltip_with_action(pane.tooltip(), &ToggleHistoryPane, None),
        SidePane::Saved => button.tooltip_with_action(pane.tooltip(), &ToggleSavedPane, None),
    }
}
