//! Tagging tables (`@name`) so their schema reaches the model with the message.

use based_ai::tools::ToolCall;
use gpui_kit::component::{
    Disableable as _, IconName, Sizable as _,
    button::{Button, ButtonVariants as _},
    input::InlineToken,
    menu::{DropdownMenu as _, PopupMenuItem},
};
use gpui_kit::{Context, IntoElement, Window, px};

use super::ChatPane;
use crate::db;

/// Menu entries shown before the list is cut; `@name` in the text covers the rest.
const MENU_MAX: usize = 300;

impl ChatPane {
    /// Loads the active connection's table names when the connection changes.
    pub(super) fn ensure_tables(&mut self, cx: &mut Context<Self>) {
        let (Some(tools), Some(id)) = (
            self.connection_tools(cx),
            self.active().map(|c| c.entry.read(cx).id.clone()),
        ) else {
            self.tables.clear();
            self.tables_for = None;
            return;
        };
        if self.tables_for.as_ref() == Some(&id) {
            return;
        }
        self.tables_for = Some(id.clone());
        self.tables.clear();
        cx.spawn(async move |this, cx| {
            let call = ToolCall {
                id: String::new(),
                name: "list_tables".into(),
                args: serde_json::json!({}),
            };
            let Ok(result) = db::run_infallible(cx, async move { tools.call(&call).await }).await
            else {
                return;
            };
            let _ = this.update(cx, |this, cx| {
                if this.tables_for.as_ref() == Some(&id) && !result.is_error {
                    this.tables = result.content.lines().map(str::to_owned).collect();
                    cx.notify();
                }
            });
        })
        .detach();
    }

    /// Tokens in the composer plus bare `@name` mentions in `text`, deduplicated.
    pub(super) fn take_tags(&self, text: &str, cx: &Context<Self>) -> Vec<String> {
        let mut tags: Vec<String> = Vec::new();
        let tokens = self.input.read(cx).tokens().to_vec();
        for span in &tokens {
            let id = span.token().id().to_string();
            if !tags.contains(&id) {
                tags.push(id);
            }
        }
        for word in text.split_whitespace().filter_map(|w| w.strip_prefix('@')) {
            let word = word.trim_end_matches([',', ';', ':', '?', '!', ')', '.']);
            if let Some(name) = self.tables.iter().find(|t| t.eq_ignore_ascii_case(word))
                && !tags.contains(name)
            {
                tags.push(name.clone());
            }
        }
        tags
    }

    /// Inserts `table` as an atomic `@table` token at the caret.
    fn add_tag(&mut self, table: &str, window: &mut Window, cx: &mut Context<Self>) {
        let token = InlineToken::new(table.to_owned(), format!("@{table}"));
        self.input.update(cx, |input, cx| {
            let _ = input.replace_with_token(token, window, cx);
        });
        self.focus(window, cx);
    }

    pub(super) fn tag_button(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let tables: Vec<String> = self.tables.iter().take(MENU_MAX).cloned().collect();
        let pane = cx.weak_entity();
        Button::new("chat-tag")
            .ghost()
            .small()
            .icon(IconName::Plus)
            .tooltip("Tag a table")
            .disabled(tables.is_empty())
            .dropdown_menu(move |menu, _, _| {
                let mut menu = menu.max_h(px(280.0)).scrollable(true);
                for table in &tables {
                    let (table, pane) = (table.clone(), pane.clone());
                    menu = menu.item(PopupMenuItem::new(table.clone()).on_click(
                        move |_, window, cx| {
                            let _ = pane.update(cx, |p, cx| p.add_tag(&table, window, cx));
                        },
                    ));
                }
                menu
            })
    }
}
