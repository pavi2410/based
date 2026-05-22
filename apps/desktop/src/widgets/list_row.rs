//! Shared `ListItem` row chrome for selectable lists.

use gpui::{ElementId, Hsla, IntoElement, ParentElement, SharedString, div, prelude::*, px};
use gpui_component::{Selectable, StyledExt, h_flex, list::ListItem};

/// Command palette result row (connection hint + primary label).
pub fn palette_result_row(
    id: impl Into<ElementId>,
    selected: bool,
    conn_label: SharedString,
    label: SharedString,
    muted: Hsla,
    fg: Hsla,
) -> ListItem {
    ListItem::new(id)
        .selected(selected)
        .px(px(12.0))
        .py(px(8.0))
        .cursor_pointer()
        .child(
            h_flex()
                .gap_2()
                .child(div().text_xs().text_color(muted).child(conn_label))
                .child(div().flex_1().text_sm().text_color(fg).child(label)),
        )
}

/// Schema browser row: badge column + label.
pub fn schema_object_row(
    id: impl Into<ElementId>,
    selected: bool,
    badge: impl Into<SharedString>,
    label: impl Into<SharedString>,
    muted: Hsla,
    fg: Hsla,
) -> ListItem {
    ListItem::new(id)
        .selected(selected)
        .px(px(8.0))
        .py(px(4.0))
        .cursor_pointer()
        .child(
            h_flex()
                .gap(px(6.0))
                .child(
                    div()
                        .text_xs()
                        .text_color(muted)
                        .w(px(28.0))
                        .child(badge.into()),
                )
                .child(div().text_sm().text_color(fg).child(label.into())),
        )
}
