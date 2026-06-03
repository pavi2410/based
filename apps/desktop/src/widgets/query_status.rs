//! Query status widgets — toolbar indicator and error card for SQL query panels.

use gpui::{prelude::*, *};
use gpui_component::{
    ActiveTheme, Icon, IconName, Sizable,
    button::{Button, ButtonVariants},
    h_flex,
    spinner::Spinner,
};

use crate::widgets::metadata_pill;

/// Engine-agnostic representation of a query execution state.
///
/// Each engine converts its own `QueryStatus` enum into this type before
/// calling [`query_status_indicator`] or [`query_error_card`].
#[derive(Clone)]
pub enum QueryStatusDisplay {
    Idle,
    Running,
    Done {
        rows: usize,
        /// Present for engines that report affected-row counts (e.g. Postgres DML).
        affected: Option<u64>,
        elapsed_ms: u64,
    },
    Error(SharedString),
}

/// Right-aligned status cluster shown at the end of a query-editor toolbar.
pub fn query_status_indicator(status: &QueryStatusDisplay, cx: &mut App) -> AnyElement {
    let muted = cx.theme().muted_foreground;
    match status {
        QueryStatusDisplay::Idle => h_flex()
            .gap(px(6.0))
            .items_center()
            .child(
                div()
                    .w(px(6.0))
                    .h(px(6.0))
                    .rounded_full()
                    .bg(muted.opacity(0.55)),
            )
            .child(div().text_xs().text_color(muted).child("Ready"))
            .into_any_element(),
        QueryStatusDisplay::Running => h_flex()
            .gap(px(6.0))
            .items_center()
            .child(Spinner::new().xsmall().color(cx.theme().primary))
            .child(div().text_xs().text_color(muted).child("Running"))
            .into_any_element(),
        QueryStatusDisplay::Done {
            rows,
            affected,
            elapsed_ms,
        } => {
            let success = cx.theme().success_foreground;
            let mut row = h_flex()
                .gap(px(6.0))
                .items_center()
                .child(
                    Icon::new(IconName::CircleCheck)
                        .text_color(success)
                        .xsmall(),
                )
                .child(metadata_pill("rows", rows.to_string(), cx));
            if let Some(n) = affected {
                row = row.child(metadata_pill("affected", n.to_string(), cx));
            }
            row.child(metadata_pill("time", format!("{elapsed_ms} ms"), cx))
                .into_any_element()
        }
        QueryStatusDisplay::Error(_) => {
            let danger = cx.theme().danger_foreground;
            h_flex()
                .gap(px(6.0))
                .items_center()
                .child(
                    Icon::new(IconName::TriangleAlert)
                        .text_color(danger)
                        .xsmall(),
                )
                .child(
                    div()
                        .text_xs()
                        .font_weight(FontWeight::SEMIBOLD)
                        .text_color(danger)
                        .child("Failed"),
                )
                .into_any_element()
        }
    }
}

/// Danger-bordered mono error card with a copy button.
///
/// Wrap the returned element in a `div().flex_1().min_h(px(0.0)).p_3()` to get
/// the standard padded full-height messages-pane layout.
pub fn query_error_card(
    id: impl Into<ElementId>,
    error: SharedString,
    cx: &App,
) -> impl IntoElement {
    let theme = cx.theme();
    let err_fg = theme.danger_foreground;
    let danger_bg = theme.danger.opacity(0.06);
    let danger_border = theme.danger.opacity(0.20);
    let mono = theme.mono_font_family.clone();
    let copy_text = error.clone();
    h_flex()
        .id(id.into())
        .p_3()
        .gap_2()
        .items_start()
        .rounded(px(6.0))
        .border_1()
        .border_color(danger_border)
        .bg(danger_bg)
        .child(
            div().mt(px(2.0)).child(
                Icon::new(IconName::TriangleAlert)
                    .text_color(err_fg)
                    .xsmall(),
            ),
        )
        .child(
            div()
                .flex_1()
                .min_w_0()
                .text_xs()
                .font_family(mono)
                .text_color(err_fg)
                .child(error),
        )
        .child(
            Button::new("error-copy")
                .ghost()
                .xsmall()
                .icon(IconName::Copy)
                .tooltip(SharedString::from("Copy error"))
                .on_click(move |_, _, cx| {
                    cx.write_to_clipboard(ClipboardItem::new_string(copy_text.to_string()));
                }),
        )
}
