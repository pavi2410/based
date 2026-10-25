//! Under each finished reply: copy, fork, and when it was written.

use std::ops::Range;

use based_ai::Role;
use based_ai::stamp::{ago, now_ms, took};
use gpui_kit::component::{
    ActiveTheme as _, Icon, IconName, Sizable as _,
    button::{Button, ButtonVariants as _},
    h_flex,
    tooltip::Tooltip,
};
use gpui_kit::{
    AnyElement, App, ClipboardItem, Entity, InteractiveElement as _, IntoElement, ParentElement,
    StatefulInteractiveElement as _, Styled, div,
};

use super::{ChatPane, style};

const FORK_ICON: &str = "icons/git-branch.svg";

impl ChatPane {
    /// Reply text of a group: its assistant messages, in order.
    fn reply_text(&self, range: &Range<usize>) -> String {
        self.turns[range.clone()]
            .iter()
            .filter(|t| t.role == Role::Assistant && !t.content.is_empty())
            .map(|t| t.content.as_str())
            .collect::<Vec<_>>()
            .join("\n\n")
    }

    /// `None` when the group has no reply text to act on.
    pub(super) fn group_footer(
        &self,
        range: Range<usize>,
        this: &Entity<Self>,
        cx: &App,
    ) -> Option<AnyElement> {
        let text = self.reply_text(&range);
        if text.is_empty() {
            return None;
        }
        let ix = range.start;
        let upto = range.end - 1;
        let finished = self.turns[range.clone()].iter().filter_map(|t| t.at).max();
        let started = range
            .start
            .checked_sub(1)
            .and_then(|i| self.turns.get(i))
            .filter(|t| t.role == Role::User)
            .and_then(|t| t.at);

        let fork = this.clone();
        let mut row = h_flex()
            .gap_1()
            .items_center()
            .child(
                Button::new(("chat-copy", ix))
                    .ghost()
                    .xsmall()
                    .icon(IconName::Copy)
                    .tooltip("Copy")
                    .on_click(move |_, _, cx| {
                        cx.write_to_clipboard(ClipboardItem::new_string(text.clone()));
                    }),
            )
            .child(
                Button::new(("chat-fork", ix))
                    .ghost()
                    .xsmall()
                    .icon(Icon::empty().path(FORK_ICON))
                    .tooltip("Fork chat from here")
                    .on_click(move |_, _, cx| {
                        fork.update(cx, |p, cx| p.fork(upto, cx));
                    }),
            );
        if let Some(done) = finished {
            let label = ago(now_ms(), done);
            let worked = started.map(|s| format!("Worked for {}", took(done.saturating_sub(s))));
            let stamp = div()
                .id(("chat-ago", ix))
                .text_size(style::small(cx))
                .text_color(cx.theme().muted_foreground)
                .child(label);
            row = row.child(match worked {
                Some(tip) => stamp
                    .tooltip(move |window, app| Tooltip::new(tip.clone()).build(window, app))
                    .into_any_element(),
                None => stamp.into_any_element(),
            });
        }
        Some(row.into_any_element())
    }
}
