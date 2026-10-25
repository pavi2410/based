//! Right-hand workspace pane: enum + shared 320 px column chrome.
//!
//! The pane swaps between Inspector / History / Saved based on the activity
//! rail. Each variant renders its own body; this module supplies the surrounding
//! border, header strip, and width so every pane looks consistent.

use crate::app::prefs::ui_font_family;
use gpui_kit::component::{ActiveTheme, IconName, h_flex, v_flex};
use gpui_kit::{
    App, AppContext as _, Context, Empty, FontWeight, InteractiveElement as _, IntoElement,
    ParentElement, Render, StatefulInteractiveElement as _, Styled, Window, div, px,
};

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum SidePane {
    Inspector,
    History,
    Saved,
    Chat,
}

impl SidePane {
    pub const ALL: [Self; 4] = [Self::Inspector, Self::History, Self::Saved, Self::Chat];

    pub fn label(self) -> &'static str {
        match self {
            Self::Inspector => "INSPECTOR",
            Self::History => "HISTORY",
            Self::Saved => "SAVED",
            Self::Chat => "ASSISTANT",
        }
    }

    pub fn icon(self) -> IconName {
        match self {
            Self::Inspector => IconName::Inspector,
            Self::History => IconName::Inbox,
            Self::Saved => IconName::Star,
            Self::Chat => IconName::Bot,
        }
    }

    pub fn tooltip(self) -> &'static str {
        match self {
            Self::Inspector => "Inspector",
            Self::History => "Query history",
            Self::Saved => "Saved queries",
            Self::Chat => "AI assistant",
        }
    }
}

pub const SIDE_PANE_WIDTH: f32 = 320.0;
const MIN_WIDTH: f32 = 260.0;
const MAX_WIDTH: f32 = 720.0;
/// Space kept for the editor when the pane is dragged wide.
const MIN_CENTER: f32 = 360.0;

/// Drag payload for the pane's resize handle.
#[derive(Clone)]
pub struct PaneResize;

impl Render for PaneResize {
    fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
        Empty
    }
}

/// Pane width for a pointer at `pointer_x` in a window `window_w` wide.
pub fn width_for_pointer(window_w: f32, pointer_x: f32) -> f32 {
    let max = MAX_WIDTH.min(window_w - MIN_CENTER).max(MIN_WIDTH);
    (window_w - pointer_x).clamp(MIN_WIDTH, max)
}

/// Wrap a pane body with the shared column chrome (header + body); drag its left edge to resize.
pub fn render_side_pane(
    active: SidePane,
    body: impl IntoElement,
    width: f32,
    cx: &App,
) -> impl IntoElement {
    let border = cx.theme().border;
    let muted = cx.theme().muted_foreground;

    v_flex()
        .relative()
        .w(px(width))
        .h_full()
        .flex_shrink_0()
        .min_h_0()
        .border_l_1()
        .border_color(border)
        .bg(cx.theme().background)
        .child(
            div()
                .id("side-pane-resize")
                .absolute()
                .top_0()
                .bottom_0()
                .left(px(-3.0))
                .w(px(6.0))
                .cursor_col_resize()
                .on_drag(PaneResize, |drag, _, _, cx| cx.new(|_| drag.clone())),
        )
        .child(
            h_flex()
                .h(px(38.0))
                .px_3()
                .items_center()
                .border_b_1()
                .border_color(border.opacity(0.86))
                .flex_shrink_0()
                .child(
                    div()
                        .text_xs()
                        .font_weight(FontWeight::BOLD)
                        .font_family(ui_font_family(cx))
                        .text_color(muted)
                        .child(active.label()),
                ),
        )
        .child(div().flex_1().min_h_0().child(body))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn width_follows_the_pointer_within_limits() {
        assert_eq!(width_for_pointer(1600.0, 1200.0), 400.0);
        assert_eq!(width_for_pointer(1600.0, 1500.0), MIN_WIDTH);
        assert_eq!(width_for_pointer(1600.0, 100.0), MAX_WIDTH);
        assert_eq!(width_for_pointer(800.0, 100.0), 440.0);
    }
}
