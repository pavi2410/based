//! Shared `ListItem` row chrome for selectable lists.

use gpui_kit::component::{Icon, IconName, Sizable as _, h_flex, list::ListItem};
use gpui_kit::{ElementId, Hsla, IntoElement, ParentElement, SharedString, div, prelude::*, px};

use crate::widgets::{SCHEMA_ROW_ICON_SIZE, SIDEBAR_INSET};

/// Typography and colors for schema browser list rows.
pub struct SchemaRowStyle {
    pub fg: Hsla,
    pub icon_color: Hsla,
    pub row_py: f32,
    pub row_gap: f32,
}

fn schema_object_row_inner(
    kind_icon: IconName,
    label: SharedString,
    style: SchemaRowStyle,
    actions: Option<gpui_kit::AnyElement>,
) -> impl IntoElement {
    let SchemaRowStyle {
        fg,
        icon_color,
        row_py: _,
        row_gap,
    } = style;
    let mut row = h_flex()
        .w_full()
        .gap(px(row_gap))
        .items_center()
        .child(
            div()
                .flex_shrink_0()
                .w(px(SCHEMA_ROW_ICON_SIZE))
                .flex()
                .items_center()
                .justify_center()
                .child(
                    Icon::new(kind_icon)
                        .text_color(icon_color)
                        .with_size(px(SCHEMA_ROW_ICON_SIZE)),
                ),
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
    kind_icon: IconName,
    label: impl Into<SharedString>,
    style: SchemaRowStyle,
) -> ListItem {
    ListItem::new(id)
        .selected(selected)
        .pl(px(SIDEBAR_INSET))
        .pr(px(SIDEBAR_INSET))
        .py(px(style.row_py))
        .cursor_pointer()
        .child(schema_object_row_inner(
            kind_icon,
            label.into(),
            style,
            None,
        ))
}
