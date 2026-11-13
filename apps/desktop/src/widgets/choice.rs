//! Typed select items: the select holds the value itself, so reading it back
//! never parses a display label.

use gpui_kit::SharedString;
use gpui_kit::component::searchable_list::SearchableListItem;

#[derive(Clone, Debug)]
pub struct Choice<T> {
    pub label: &'static str,
    pub value: T,
}

impl<T: Clone + PartialEq + 'static> SearchableListItem for Choice<T> {
    type Value = T;

    fn title(&self) -> SharedString {
        self.label.into()
    }

    fn value(&self) -> &T {
        &self.value
    }
}

/// `(value, label)` pairs into select items.
pub fn choices<T: Copy>(items: &[(T, &'static str)]) -> Vec<Choice<T>> {
    items
        .iter()
        .map(|&(value, label)| Choice { label, value })
        .collect()
}
