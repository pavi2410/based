//! Shared `ListItem` row chrome for selectable lists.

use gpui::{ElementId, Hsla, IntoElement, ParentElement, SharedString, div, prelude::*, px};
use gpui_component::{Icon, IconName, Sizable as _, h_flex, list::ListItem, v_flex};

use crate::widgets::ui::{SCHEMA_ROW_ICON_SIZE, SIDEBAR_INSET};

/// Typography and colors for schema browser list rows.
pub struct SchemaRowStyle {
    pub muted: Hsla,
    pub fg: Hsla,
    pub mono_family: SharedString,
    pub row_py: f32,
    pub row_gap: f32,
}

/// Command palette result row (connection hint + primary label + meta).
pub fn palette_result_row(
    id: impl Into<ElementId>,
    selected: bool,
    conn_label: SharedString,
    label: SharedString,
    sublabel: SharedString,
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
                .w_full()
                .gap_2()
                .items_center()
                .child(
                    div()
                        .flex_shrink_0()
                        .w(px(72.0))
                        .text_xs()
                        .text_color(muted)
                        .truncate()
                        .child(conn_label),
                )
                .child(
                    v_flex()
                        .flex_1()
                        .min_w_0()
                        .gap(px(1.0))
                        .child(div().text_sm().text_color(fg).truncate().child(label))
                        .child(
                            div()
                                .text_xs()
                                .text_color(muted.opacity(0.9))
                                .truncate()
                                .child(sublabel),
                        ),
                ),
        )
}

fn schema_object_row_inner(
    kind_icon: IconName,
    label: SharedString,
    style: SchemaRowStyle,
    actions: Option<gpui::AnyElement>,
) -> impl IntoElement {
    let SchemaRowStyle {
        muted,
        fg,
        mono_family: _,
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
                        .text_color(muted)
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

/// Schema browser row with trailing actions (e.g. inspect / insert).
pub fn schema_object_row_with_actions(
    id: impl Into<ElementId>,
    selected: bool,
    kind_icon: IconName,
    label: impl Into<SharedString>,
    style: SchemaRowStyle,
    actions: impl IntoElement,
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
            Some(actions.into_any_element()),
        ))
}
