//! Bottom tab strip used by the Postgres/SQLite query editors to switch between
//! the result table, status messages, and the EXPLAIN plan inside a single tab.

use std::rc::Rc;

use gpui::{App, ElementId, IntoElement, ParentElement, Styled, Window, div, prelude::*, px};
use gpui_component::{
    ActiveTheme, Selectable as _, Sizable as _,
    button::{Button, ButtonVariants},
    h_flex,
};

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

    fn id_suffix(self) -> &'static str {
        match self {
            Self::Results => "results",
            Self::Messages => "messages",
            Self::Explain => "explain",
        }
    }
}

/// Render the strip of `Results | Messages | Explain` ghost buttons.
///
/// `id_prefix` keeps the button element ids unique across panels (e.g. `"pg-bt"`).
/// `has_error` paints a small danger dot next to the Messages button.
/// `on_select` is invoked with the clicked tab; it is shared (`Rc`) across the
/// three click handlers so the caller can wire it with a single `cx.listener`.
pub fn result_tab_strip(
    id_prefix: &'static str,
    active: BottomTab,
    has_error: bool,
    on_select: Rc<dyn Fn(BottomTab, &mut Window, &mut App)>,
    cx: &App,
) -> impl IntoElement + use<> {
    let theme = cx.theme();
    let border = theme.border;
    let danger = theme.danger;

    let mut row = h_flex()
        .gap(px(2.0))
        .px(px(6.0))
        .py(px(2.0))
        .items_center()
        .border_b_1()
        .border_color(border.opacity(0.72))
        .bg(theme.muted.opacity(0.12));

    for tab in BottomTab::ALL {
        let cb = on_select.clone();
        let id: ElementId = format!("{id_prefix}-{}", tab.id_suffix()).into();
        let button = Button::new(id)
            .ghost()
            .small()
            .label(tab.label())
            .selected(active == tab)
            .on_click(move |_, window, cx| cb(tab, window, cx));

        row = row
            .child(button)
            .when(tab == BottomTab::Messages && has_error, |row| {
                row.child(
                    div()
                        .w(px(6.0))
                        .h(px(6.0))
                        .rounded_full()
                        .bg(danger)
                        .ml(px(-2.0)),
                )
            });
    }

    row
}
