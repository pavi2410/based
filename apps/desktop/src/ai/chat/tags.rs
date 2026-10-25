//! Tagging tables (`@name`) so their schema reaches the model with the message.

use std::mem::take;

use based_ai::tools::ToolCall;
use gpui_kit::component::{
    Disableable as _, IconName, Sizable as _,
    button::{Button, ButtonVariants as _},
    menu::{DropdownMenu as _, PopupMenuItem},
};
use gpui_kit::{Context, IntoElement, px};

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

    /// Chips plus `@name` mentions in `text`, deduplicated and spelled as the database does.
    pub(super) fn take_tags(&mut self, text: &str) -> Vec<String> {
        let mut tags = take(&mut self.tags);
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

    pub(super) fn add_tag(&mut self, table: String, cx: &mut Context<Self>) {
        if !self.tags.contains(&table) {
            self.tags.push(table);
            cx.notify();
        }
    }

    pub(super) fn remove_tag(&mut self, ix: usize, cx: &mut Context<Self>) {
        if ix < self.tags.len() {
            self.tags.remove(ix);
            cx.notify();
        }
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
                    menu =
                        menu.item(PopupMenuItem::new(table.clone()).on_click(move |_, _, cx| {
                            let _ = pane.update(cx, |p, cx| p.add_tag(table.clone(), cx));
                        }));
                }
                menu
            })
    }
}

/// `text` plus ` @table` for tags it does not already mention.
pub(super) fn with_mentions(mut text: String, tags: &[String]) -> String {
    let lower = text.to_lowercase();
    let missing: Vec<&String> = tags
        .iter()
        .filter(|t| !lower.contains(&format!("@{}", t.to_lowercase())))
        .collect();
    for tag in missing {
        text.push_str(" @");
        text.push_str(tag);
    }
    text
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn mentions_are_appended_once() {
        let tags = vec!["users".to_owned(), "orders".to_owned()];
        assert_eq!(
            with_mentions("join @Users with it".into(), &tags),
            "join @Users with it @orders"
        );
    }
}
