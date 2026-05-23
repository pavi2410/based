//! Cross-panel tab open requests (query editor → workspace).

use gpui::{BorrowAppContext, Global};

use super::TabSpec;

#[derive(Default)]
pub struct TabOpenQueue {
    pub pending: Option<TabSpec>,
}

impl Global for TabOpenQueue {}

pub fn enqueue_open_tab(spec: TabSpec, cx: &mut impl BorrowAppContext) {
    cx.update_global(|q: &mut TabOpenQueue, _| {
        q.pending = Some(spec);
    });
}
