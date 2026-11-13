//! What the assistant may see on the active connection: the chip under the composer that
//! shows the level and changes it, and the starter questions built from the schema.

use based_ai::context::ContextLevel;
use gpui_kit::component::{
    ActiveTheme, Icon, IconName, Sizable as _,
    button::{Button, ButtonVariants as _},
    h_flex,
    menu::{DropdownMenu as _, PopupMenuItem},
    v_flex,
};
use gpui_kit::{
    AnyElement, Context, InteractiveElement as _, IntoElement, ParentElement,
    StatefulInteractiveElement as _, Styled, WeakEntity, div,
};

use super::{ChatPane, style};
use crate::connection::ConnectionId;
use crate::workspace::WorkspaceRef;

/// Starters shown on an empty chat.
const STARTERS: usize = 4;

pub(super) fn level_label(level: ContextLevel) -> &'static str {
    match level {
        ContextLevel::Off => "AI off",
        ContextLevel::Schema => "Schema only",
        ContextLevel::SchemaRows => "Schema and rows",
    }
}

fn level_detail(level: ContextLevel) -> &'static str {
    match level {
        ContextLevel::Off => "Nothing is sent. Chat is disabled.",
        ContextLevel::Schema => "Table and column names, types and keys. No row values.",
        ContextLevel::SchemaRows => "Also lets the assistant read rows with read-only queries.",
    }
}

/// Starter questions for a connection, most useful first. `tables` are real names.
pub(super) fn starters(tables: &[String], level: ContextLevel) -> Vec<String> {
    let mut out = vec!["Which tables look the largest, and what is each for?".to_owned()];
    if let Some(first) = tables.first() {
        out.push(format!("Explain how {first} relates to the other tables"));
        if level == ContextLevel::SchemaRows {
            out.push(format!("Show me 10 recent rows from {first}"));
        } else {
            out.push(format!("Write a query that counts rows in {first} by day"));
        }
    }
    if let Some(second) = tables.get(1) {
        out.push(format!("How would I join {} to {second}?", tables[0]));
    }
    out.truncate(STARTERS);
    out
}

impl ChatPane {
    /// Level chip with a menu to change it. `None` when there is no connection.
    pub(super) fn access_chip(&self, cx: &mut Context<Self>) -> Option<AnyElement> {
        let ctx = self.active()?;
        let current = ctx.level;
        let id = ctx.entry.read(cx).id.clone();
        let pane: WeakEntity<Self> = cx.weak_entity();
        let muted = cx.theme().muted_foreground;
        let tone = match current {
            ContextLevel::Off => cx.theme().warning,
            _ => muted,
        };
        Some(
            Button::new("chat-access")
                .ghost()
                .xsmall()
                .icon(Icon::new(IconName::Eye).text_color(tone))
                .label(level_label(current))
                .tooltip(level_detail(current))
                .dropdown_menu(move |mut menu, _, _| {
                    for level in [
                        ContextLevel::Off,
                        ContextLevel::Schema,
                        ContextLevel::SchemaRows,
                    ] {
                        let (id, pane) = (id.clone(), pane.clone());
                        menu = menu.item(
                            PopupMenuItem::new(format!(
                                "{} · {}",
                                level_label(level),
                                level_detail(level)
                            ))
                            .checked(level == current)
                            .on_click(move |_, _, cx| {
                                let _ = pane.update(cx, |p, cx| p.set_access(&id, level, cx));
                            }),
                        );
                    }
                    menu
                })
                .into_any_element(),
        )
    }

    pub(super) fn set_access(
        &mut self,
        id: &ConnectionId,
        level: ContextLevel,
        cx: &mut Context<Self>,
    ) {
        let Some(ws) = cx.try_global::<WorkspaceRef>().map(|w| w.0.clone()) else {
            return;
        };
        let id = id.clone();
        // The workspace owns the project files; defer so this click can finish first.
        cx.defer(move |cx| {
            let result = ws.update(cx, |w, cx| w.set_connection_ai(&id, level, cx));
            if let Err(message) = result {
                log::warn!("could not change AI access: {message}");
                crate::workspace::notify::push_error(cx, "Couldn't change AI access", message);
            }
        });
    }

    /// Empty chat: what the assistant knows, and one-click questions.
    pub(super) fn empty_state(&self, cx: &mut Context<Self>) -> AnyElement {
        let muted = cx.theme().muted_foreground;
        let Some(ctx) = self.active() else {
            return super::view::muted_line("Open a database to ask about it.", cx);
        };
        if ctx.level == ContextLevel::Off {
            let id = ctx.entry.read(cx).id.clone();
            return v_flex()
                .flex_1()
                .justify_center()
                .items_start()
                .gap_2()
                .px_4()
                .child(
                    div()
                        .text_size(style::body(cx))
                        .text_color(muted)
                        .child(format!("AI is off for {}.", ctx.label)),
                )
                .child(
                    Button::new("chat-enable")
                        .primary()
                        .small()
                        .label("Enable, schema only")
                        .on_click(cx.listener(move |this, _, _, cx| {
                            this.set_access(&id, ContextLevel::Schema, cx);
                        })),
                )
                .child(div().text_size(style::small(cx)).text_color(muted).child(
                    "Sends table and column names, never row values. Saved in the connection file.",
                ))
                .into_any_element();
        }
        let known = match self.tables.len() {
            0 => String::new(),
            1 => "1 table".to_owned(),
            n => format!("{n} tables"),
        };
        let items = starters(&self.tables, ctx.level);
        v_flex()
            .flex_1()
            .justify_center()
            .gap_3()
            .px_4()
            .child(
                v_flex()
                    .gap_0p5()
                    .child(
                        div()
                            .text_size(style::title(cx))
                            .child(format!("Ask about {}", ctx.label)),
                    )
                    .child(
                        div()
                            .text_size(style::small(cx))
                            .text_color(muted)
                            .child(format!(
                                "{known}{}{}. Start with one of these, or write your own.",
                                if known.is_empty() { "" } else { " · " },
                                level_label(ctx.level).to_lowercase()
                            )),
                    ),
            )
            .child(
                v_flex()
                    .gap_1p5()
                    .children(items.into_iter().enumerate().map(|(ix, text)| {
                        let send = text.clone();
                        let theme = cx.theme();
                        let (border, hover) = (theme.border, theme.primary.opacity(0.1));
                        h_flex()
                            .id(("chat-starter", ix))
                            .px_3()
                            .py_2()
                            .rounded_lg()
                            .border_1()
                            .border_color(border)
                            .cursor_pointer()
                            .text_size(style::body(cx))
                            .hover(move |s| s.bg(hover))
                            .on_click(cx.listener(move |this, _, _, cx| {
                                this.send(send.clone(), cx);
                            }))
                            .child(text)
                    })),
            )
            .into_any_element()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn starters_use_real_table_names() {
        let tables = vec!["orders".to_owned(), "customers".to_owned()];
        let schema = starters(&tables, ContextLevel::Schema);
        assert!(schema.iter().any(|s| s.contains("orders")));
        assert!(schema.iter().all(|s| !s.contains("recent rows")));
        let rows = starters(&tables, ContextLevel::SchemaRows);
        assert!(rows.iter().any(|s| s.contains("recent rows from orders")));
        assert_eq!(starters(&[], ContextLevel::Schema).len(), 1);
    }
}
