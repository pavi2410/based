//! Saving, listing and reopening conversations.

use std::path::PathBuf;

use based_ai::history::{self, Chat, ChatMeta};
use gpui_kit::component::{
    Icon, Sizable as _,
    button::{Button, ButtonVariants as _},
    menu::{DropdownMenu as _, PopupMenuItem},
};
use gpui_kit::{AppContext as _, Context, IntoElement, px};

use super::{ChatPane, Phase};

const HISTORY_ICON: &str = "icons/history.svg";

/// Chats offered in the menu.
const MENU_MAX: usize = 30;

pub(super) fn store_dir() -> PathBuf {
    dirs::data_dir()
        .unwrap_or_else(|| PathBuf::from("."))
        .join("based")
        .join("chats")
}

impl ChatPane {
    /// Loads the saved-chat list in the background.
    pub(super) fn load_history(&self, cx: &mut Context<Self>) {
        cx.spawn(async move |this, cx| {
            let list = cx
                .background_executor()
                .spawn(async { history::list(&store_dir()) })
                .await;
            let _ = this.update(cx, |this, cx| {
                this.history = list;
                cx.notify();
            });
        })
        .detach();
    }

    /// Writes the conversation so far; call whenever it settles.
    pub(super) fn persist(&mut self, cx: &mut Context<Self>) {
        if self.turns.is_empty() {
            return;
        }
        let id = self.chat_id.get_or_insert_with(history::new_id).clone();
        let chat = Chat::new(id, self.turns.clone(), self.targets.clone());
        self.history.retain(|m| m.id != chat.id);
        self.history.insert(0, chat.meta());
        cx.background_spawn(async move {
            if let Err(e) = history::save(&store_dir(), &chat) {
                log::warn!("could not save chat: {e}");
            }
        })
        .detach();
        cx.notify();
    }

    /// Starts an empty conversation; the old one stays in history.
    pub(super) fn new_chat(&mut self, cx: &mut Context<Self>) {
        self.turns.clear();
        self.draft.clear();
        self.targets.clear();
        self.chat_id = None;
        self.trusted.clear();
        self.finish(Phase::Idle);
        self.sync(cx);
    }

    fn open_saved(&mut self, id: String, cx: &mut Context<Self>) {
        if self.chat_id.as_deref() == Some(&id) {
            return;
        }
        self.finish(Phase::Idle);
        cx.spawn(async move |this, cx| {
            let key = id.clone();
            let loaded = cx
                .background_executor()
                .spawn(async move { history::load(&store_dir(), &key) })
                .await;
            let _ = this.update(cx, |this, cx| match loaded {
                Ok(chat) => {
                    this.finish(Phase::Idle);
                    this.draft.clear();
                    this.turns = chat.messages;
                    this.targets = chat.targets;
                    this.chat_id = Some(chat.id);
                    this.sync(cx);
                }
                Err(e) => log::warn!("could not open chat {id}: {e}"),
            });
        })
        .detach();
    }

    /// Removes the open conversation from disk and starts a new one.
    fn delete_saved(&mut self, cx: &mut Context<Self>) {
        if let Some(id) = self.chat_id.clone() {
            self.history.retain(|m| m.id != id);
            cx.background_spawn(async move {
                let _ = history::delete(&store_dir(), &id);
            })
            .detach();
        }
        self.new_chat(cx);
    }

    /// Header button listing saved chats. The list is read when the menu opens, so it stays
    /// current even though the header is drawn by the workspace.
    pub fn history_button(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let pane = cx.weak_entity();
        Button::new("chat-history")
            .ghost()
            .small()
            .icon(Icon::empty().path(HISTORY_ICON))
            .tooltip("Chat history")
            .dropdown_menu(move |menu, _, cx| {
                let mut menu = menu.max_h(px(320.0)).scrollable(true);
                let Some(this) = pane.upgrade() else {
                    return menu;
                };
                let (chats, current) = {
                    let this = this.read(cx);
                    let chats: Vec<ChatMeta> =
                        this.history.iter().take(MENU_MAX).cloned().collect();
                    (chats, this.chat_id.clone())
                };
                if chats.is_empty() {
                    return menu.item(PopupMenuItem::new("No saved chats").disabled(true));
                }
                for chat in &chats {
                    let (id, pane) = (chat.id.clone(), pane.clone());
                    menu = menu.item(
                        PopupMenuItem::new(chat.title.clone())
                            .checked(current.as_deref() == Some(chat.id.as_str()))
                            .on_click(move |_, _, cx| {
                                let _ = pane.update(cx, |p, cx| p.open_saved(id.clone(), cx));
                            }),
                    );
                }
                if current.is_some() {
                    let pane = pane.clone();
                    menu = menu
                        .separator()
                        .item(
                            PopupMenuItem::new("Delete this chat").on_click(move |_, _, cx| {
                                let _ = pane.update(cx, |p, cx| p.delete_saved(cx));
                            }),
                        );
                }
                menu
            })
    }
}
