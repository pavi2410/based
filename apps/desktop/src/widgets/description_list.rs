//! Compact description list helpers for inspectors, dashboards, and schema stats.

use gpui::{prelude::*, *};
use gpui_component::{
    ActiveTheme, Sizable, StyledExt,
    description_list::{DescriptionItem, DescriptionList},
    v_flex,
};

use crate::app::prefs;

/// Horizontal key-value list for narrow panels (e.g. the right inspector).
pub fn compact_description_list_horizontal(
    rows: impl IntoIterator<Item = (impl Into<SharedString>, impl Into<SharedString>)>,
) -> DescriptionList {
    let items: Vec<DescriptionItem> = rows
        .into_iter()
        .map(|(label, value)| {
            let label: SharedString = label.into();
            let value: SharedString = value.into();
            DescriptionItem::new(label).value(value)
        })
        .collect();
    // `DescriptionList` defaults to 3 columns per row, which character-wraps inside
    // the 320 px Inspector side pane. Force one item per row so labels and values
    // get the full column width.
    DescriptionList::horizontal()
        .small()
        .bordered(false)
        .columns(1)
        .label_width(px(88.0))
        .children(items)
}

/// Vertical key-value list for dashboard cards and schema stats.
pub fn compact_description_list_vertical(
    rows: impl IntoIterator<Item = (impl Into<SharedString>, impl Into<SharedString>)>,
    bordered: bool,
) -> DescriptionList {
    let items: Vec<DescriptionItem> = rows
        .into_iter()
        .map(|(label, value)| {
            let label: SharedString = label.into();
            let value: SharedString = value.into();
            DescriptionItem::new(label).value(value)
        })
        .collect();
    DescriptionList::vertical()
        .small()
        .bordered(bordered)
        .children(items)
}

/// Inspector sidebar section: mono title + compact horizontal description list.
pub fn inspector_description_section(
    title: &'static str,
    rows: impl IntoIterator<Item = (&'static str, impl Into<SharedString>)>,
    cx: &mut App,
) -> impl IntoElement {
    let items: Vec<(SharedString, SharedString)> = rows
        .into_iter()
        .map(|(label, value)| (label.into(), value.into()))
        .collect();
    v_flex()
        .gap_2()
        .child(
            div()
                .text_xs()
                .font_bold()
                .font_family(prefs::ui_font_family(cx))
                .font_weight(prefs::ui_font_weight(cx))
                .text_color(cx.theme().muted_foreground)
                .child(title),
        )
        .child(compact_description_list_horizontal(items))
}
