//! Shared `ListItem` row chrome for selectable lists.

use gpui::{ElementId, Hsla, IntoElement, ParentElement, SharedString, div, prelude::*, px};
use gpui_component::{Icon, IconName, Sizable as _, h_flex, list::ListItem};

use crate::widgets::{SCHEMA_ROW_ICON_SIZE, SIDEBAR_INSET};

/// Fixed row height for command palette list items.
const PALETTE_ROW_H: f32 = 28.0;

/// Typography and colors for schema browser list rows.
pub struct SchemaRowStyle {
    pub muted: Hsla,
    pub fg: Hsla,
    pub mono_family: SharedString,
    pub row_py: f32,
    pub row_gap: f32,
}

/// Command palette result row — single-line label with trailing meta (VS Code style).
pub fn palette_result_row(
    id: impl Into<ElementId>,
    selected: bool,
    conn_label: SharedString,
    label: SharedString,
    sublabel: SharedString,
    muted: Hsla,
    fg: Hsla,
) -> ListItem {
    let meta: SharedString = if conn_label.is_empty() {
        sublabel
    } else if sublabel.contains(conn_label.as_ref()) {
        sublabel
    } else {
        format!("{conn_label} · {sublabel}").into()
    };
    ListItem::new(id)
        .selected(selected)
        .h(px(PALETTE_ROW_H))
        .overflow_hidden()
        .px(px(12.0))
        .py(px(0.0))
        .cursor_pointer()
        .child(
            h_flex()
                .w_full()
                .h_full()
                .gap_2()
                .items_center()
                .overflow_hidden()
                .child(
                    div()
                        .flex_1()
                        .min_w_0()
                        .overflow_hidden()
                        .text_sm()
                        .text_color(fg)
                        .truncate()
                        .child(label),
                )
                .child(
                    div()
                        .flex_shrink_0()
                        .max_w(px(220.0))
                        .overflow_hidden()
                        .text_xs()
                        .text_color(muted)
                        .truncate()
                        .child(meta),
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
