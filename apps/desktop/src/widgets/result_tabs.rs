//! Bottom tab strip used by the Postgres/SQLite query editors to switch between
//! the result table, status messages, and the EXPLAIN plan inside a single tab.

use std::rc::Rc;

use gpui_kit::component::{ActiveTheme, h_flex};
use gpui_kit::{
    AnyElement, App, ElementId, IntoElement, ParentElement, Styled, Window, div, prelude::*, px,
};

/// A tab in the strip: one per result set, then Messages and Explain.
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub enum StripTab {
    Result(usize),
    Messages,
    Explain,
}

#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub enum BottomTab {
    Results,
    Messages,
    Explain,
}

impl BottomTab {
    pub const ALL: [Self; 3] = [Self::Results, Self::Messages, Self::Explain];

    pub fn label(self) -> &'static str {
        match self {
            Self::Results => "Results",
            Self::Messages => "Messages",
            Self::Explain => "Explain",
        }
    }
}

/// Render the result header: `Results | Messages | Explain` tabs with an accent underline,
/// a row-count badge on Results and a danger dot on Messages after a failure, then
/// `trailing` (stats, export) at the right edge.
///
/// `id_prefix` keeps element ids unique across panels (e.g. `"pg-bt"`).
/// `on_select` is shared (`Rc`) across the tab click handlers so the caller can wire it
/// with a single `cx.listener`.
#[allow(clippy::too_many_arguments)]
pub fn result_tab_strip(
    id_prefix: &str,
    active: BottomTab,
    has_error: bool,
    result_counts: Vec<usize>,
    active_result: usize,
    trailing: Option<AnyElement>,
    on_select: Rc<dyn Fn(StripTab, &mut Window, &mut App)>,
    cx: &App,
) -> impl IntoElement + use<> {
    let theme = cx.theme();
    let (border, danger, muted, fg, accent) = (
        theme.border,
        theme.danger,
        theme.muted_foreground,
        theme.foreground,
        theme.primary,
    );

    // One tab per result set (just "Results" when there is at most one), then the rest.
    let mut tabs: Vec<(StripTab, String, Option<usize>, bool)> = Vec::new();
    if result_counts.len() <= 1 {
        tabs.push((
            StripTab::Result(0),
            "Results".into(),
            result_counts.first().copied(),
            active == BottomTab::Results,
        ));
    } else {
        for (i, n) in result_counts.iter().enumerate() {
            tabs.push((
                StripTab::Result(i),
                format!("Result {}", i + 1),
                Some(*n),
                active == BottomTab::Results && active_result == i,
            ));
        }
    }
    tabs.push((
        StripTab::Messages,
        "Messages".into(),
        None,
        active == BottomTab::Messages,
    ));
    tabs.push((
        StripTab::Explain,
        "Explain".into(),
        None,
        active == BottomTab::Explain,
    ));

    let mut row = h_flex()
        .gap(px(2.0))
        .px(px(8.0))
        .min_h(px(34.0))
        .items_center()
        .border_b_1()
        .border_color(border.opacity(0.72))
        .bg(theme.muted.opacity(0.12));

    for (n, (tab, title, count, selected)) in tabs.into_iter().enumerate() {
        let cb = on_select.clone();
        let id: ElementId = format!("{id_prefix}-tab-{n}").into();
        let mut label = h_flex().gap(px(6.0)).items_center().child(title);
        if let Some(n) = count {
            label = label.child(
                div()
                    .px(px(6.0))
                    .rounded_full()
                    .border_1()
                    .border_color(border)
                    .text_size(px(10.0))
                    .text_color(muted)
                    .child(n.to_string()),
            );
        }
        if tab == StripTab::Messages && has_error {
            label = label.child(div().size(px(6.0)).rounded_full().bg(danger));
        }
        row = row.child(
            div()
                .id(id)
                .relative()
                .px(px(10.0))
                .py(px(8.0))
                .cursor_pointer()
                .text_xs()
                .font_weight(gpui_kit::FontWeight::MEDIUM)
                .text_color(if selected { fg } else { muted })
                .hover(move |s| s.text_color(fg))
                .child(label)
                .when(selected, |d| {
                    d.child(
                        div()
                            .absolute()
                            .left(px(8.0))
                            .right(px(8.0))
                            .bottom(px(-1.0))
                            .h(px(2.0))
                            .rounded_full()
                            .bg(accent),
                    )
                })
                .on_click(move |_, window, cx| cb(tab, window, cx)),
        );
    }

    row.child(div().flex_1()).children(trailing)
}
