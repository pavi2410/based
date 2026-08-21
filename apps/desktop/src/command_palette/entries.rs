use gpui::{AnyElement, App, Entity, SharedString, Window, div, prelude::*, px};
use gpui_component::command::{Command, CommandGroup, CommandItem, CommandState};
use gpui_component::{ActiveTheme, IndexPath, h_flex, v_flex};

use crate::widgets::kbd;

use super::format;
use super::types::{PaletteResult, PaletteSection};

pub fn command_for_sections(
    state: &Entity<CommandState>,
    sections: &[PaletteSection],
    on_query: impl Fn(&str, &mut Window, &mut App) + 'static,
    on_confirm: impl Fn(IndexPath, &mut Window, &mut App) + 'static,
    on_cancel: impl Fn(&mut Window, &mut App) + 'static,
) -> Command {
    let mut command = Command::new(state)
        .filterable(false)
        .placeholder("Search tables, queries, history…")
        .max_h(px(360.))
        .w(px(560.))
        .on_query(on_query)
        .on_confirm(on_confirm)
        .on_cancel(on_cancel)
        .empty(|_, _, cx| {
            v_flex()
                .w_full()
                .items_center()
                .gap_2()
                .py_6()
                .text_sm()
                .text_color(cx.theme().muted_foreground)
                .child("No results found.")
        })
        .footer(|_, _, cx| command_footer(cx));
    for section in sections {
        command = command.group(
            CommandGroup::new()
                .label(section.heading)
                .items(section.items.iter().map(command_item)),
        );
    }
    command
}

fn command_item(result: &PaletteResult) -> CommandItem {
    let label: SharedString = format::palette_single_line(&result.label, 120).into();
    let meta: SharedString = format::palette_meta(&result.conn_label, &result.sublabel).into();
    CommandItem::new().label(label.clone()).child(move |_, cx| {
        h_flex()
            .w_full()
            .gap_2()
            .items_center()
            .overflow_hidden()
            .child(
                div()
                    .flex_1()
                    .min_w_0()
                    .overflow_hidden()
                    .text_sm()
                    .truncate()
                    .child(label.clone()),
            )
            .child(
                div()
                    .flex_shrink_0()
                    .max_w(px(220.0))
                    .overflow_hidden()
                    .text_xs()
                    .text_color(cx.theme().muted_foreground)
                    .truncate()
                    .child(meta.clone()),
            )
    })
}

fn command_footer(cx: &mut App) -> AnyElement {
    let muted = cx.theme().muted_foreground;
    let hint = |stroke: &str| kbd(stroke).outline().text_color(cx.theme().foreground);
    h_flex()
        .flex_shrink_0()
        .w_full()
        .px_3()
        .py_2()
        .gap_2()
        .items_center()
        .border_t_1()
        .border_color(cx.theme().border)
        .text_xs()
        .text_color(muted)
        .child(hint("up"))
        .child(hint("down"))
        .child("navigate")
        .child("·")
        .child(hint("enter"))
        .child("open")
        .child("·")
        .child(hint("escape"))
        .child("dismiss")
        .into_any_element()
}
