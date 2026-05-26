//! Right-hand workspace pane: enum + shared 320 px column chrome.
//!
//! The pane swaps between Inspector / History / Saved based on the activity
//! rail. Each variant renders its own body; this module supplies the surrounding
//! border, header strip, and width so every pane looks consistent.

use gpui::{App, FontWeight, IntoElement, ParentElement, Styled, div, px};
use gpui_component::{ActiveTheme, IconName, h_flex, v_flex};

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum SidePane {
    Inspector,
    History,
    Saved,
}

impl SidePane {
    pub const ALL: [Self; 3] = [Self::Inspector, Self::History, Self::Saved];

    pub fn label(self) -> &'static str {
        match self {
            Self::Inspector => "INSPECTOR",
            Self::History => "HISTORY",
            Self::Saved => "SAVED",
        }
    }

    pub fn icon(self) -> IconName {
        match self {
            Self::Inspector => IconName::Inspector,
            Self::History => IconName::Inbox,
            Self::Saved => IconName::Star,
        }
    }

    pub fn tooltip(self) -> &'static str {
        match self {
            Self::Inspector => "Inspector",
            Self::History => "Query history",
            Self::Saved => "Saved queries",
        }
    }
}

pub const SIDE_PANE_WIDTH: f32 = 320.0;

/// Wrap a pane body with the shared 320 px column chrome (header + scroll area).
pub fn render_side_pane(active: SidePane, body: impl IntoElement, cx: &App) -> impl IntoElement {
    let border = cx.theme().border;
    let muted = cx.theme().muted_foreground;

    v_flex()
        .w(px(SIDE_PANE_WIDTH))
        .h_full()
        .flex_shrink_0()
        .min_h_0()
        .border_l_1()
        .border_color(border)
        .bg(cx.theme().background)
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
                        .font_family(cx.theme().mono_font_family.clone())
                        .text_color(muted)
                        .child(active.label()),
                ),
        )
        .child(div().flex_1().min_h_0().child(body))
}
