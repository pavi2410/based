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

const SCHEMA_BADGE_W: f32 = 40.0;

fn schema_object_row_inner(
    badge: SharedString,
    label: SharedString,
    muted: Hsla,
    fg: Hsla,
    mono_family: SharedString,
    actions: Option<gpui::AnyElement>,
) -> impl IntoElement {
    let mut row = h_flex()
        .w_full()
        .gap(px(6.0))
        .items_center()
        .child(
            div()
                .flex_shrink_0()
                .w(px(SCHEMA_BADGE_W))
                .text_xs()
                .font_family(mono_family)
                .text_color(muted)
                .truncate()
                .child(badge),
        )
        .child(
            div()
                .flex_1()
                .min_w_0()
                .text_sm()
                .text_color(fg)
                .truncate()
                .child(label),
        );
    if let Some(actions) = actions {
        row = row.child(actions);
    }
    row
}

/// Schema browser row: badge column + label.
pub fn schema_object_row(
    id: impl Into<ElementId>,
    selected: bool,
    badge: impl Into<SharedString>,
    label: impl Into<SharedString>,
    muted: Hsla,
    fg: Hsla,
    mono_family: SharedString,
) -> ListItem {
    ListItem::new(id)
        .selected(selected)
        .px(px(8.0))
        .py(px(4.0))
        .cursor_pointer()
        .child(schema_object_row_inner(
            badge.into(),
            label.into(),
            muted,
            fg,
            mono_family,
            None,
        ))
}

/// Schema browser row with trailing actions (e.g. inspect / insert).
pub fn schema_object_row_with_actions(
    id: impl Into<ElementId>,
    selected: bool,
    badge: impl Into<SharedString>,
    label: impl Into<SharedString>,
    muted: Hsla,
    fg: Hsla,
    mono_family: SharedString,
    actions: impl IntoElement,
) -> ListItem {
    ListItem::new(id)
        .selected(selected)
        .px(px(8.0))
        .py(px(4.0))
        .cursor_pointer()
        .child(schema_object_row_inner(
            badge.into(),
            label.into(),
            muted,
            fg,
            mono_family,
            Some(actions.into_any_element()),
        ))
}
