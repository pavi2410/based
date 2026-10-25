//! The user message that started the visible part of the conversation, pinned to the top.

use based_ai::Role;
use gpui_kit::{AnyElement, App, IntoElement, ParentElement, Styled, div, px};

use super::group::turn_groups;
use super::{ChatPane, view};

/// Lines of the pinned message shown.
const LINES: usize = 2;

impl ChatPane {
    /// The pinned message, once its row has scrolled (partly) out of view.
    pub(super) fn sticky_user(&self, cx: &App) -> Option<AnyElement> {
        let top = self.scroller.read(cx).logical_scroll_top();
        let groups = turn_groups(&self.turns);
        // `item_ix` equals the row count right after a jump to the end, before layout settles.
        let last = top.item_ix.min(groups.len().checked_sub(1)?);
        if top.item_ix >= self.rows() {
            return None;
        }
        let user = groups[..=last]
            .iter()
            .rposition(|g| self.turns[g.start].role == Role::User)?;
        if user == top.item_ix && top.offset_in_item <= px(0.0) {
            return None;
        }
        Some(
            div()
                .w_full()
                .pb_2()
                .child(view::user_row(
                    &self.turns[groups[user].start],
                    Some(LINES),
                    cx,
                ))
                .into_any_element(),
        )
    }
}
