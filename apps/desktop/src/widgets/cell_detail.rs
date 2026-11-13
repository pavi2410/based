//! Floating cell inspector: column, inferred type label, monospace body.

use gpui_kit::component::{ActiveTheme, h_flex, scroll::ScrollableElement, v_flex};
use gpui_kit::{Context, IntoElement, MouseButton, Render, Window, div, prelude::*, px};

use based_core::Value;

use crate::app::prefs;

/// Inspector body for a value: JSON pretty-printed, everything else as the grid shows it.
pub fn inspector_text(value: &Value) -> String {
    match value {
        Value::Json(s) => serde_json::from_str::<serde_json::Value>(s)
            .ok()
            .and_then(|v| serde_json::to_string_pretty(&v).ok())
            .unwrap_or_else(|| s.to_string()),
        other => other.display(),
    }
}

pub struct CellDetail {
    pub column: String,
    pub value: Value,
    pub visible: bool,
}

impl Default for CellDetail {
    fn default() -> Self {
        Self {
            column: String::new(),
            value: Value::Null,
            visible: false,
        }
    }
}

impl CellDetail {
    pub fn show(&mut self, column: String, value: Value) {
        self.column = column;
        self.value = value;
        self.visible = true;
    }

    pub fn hide(&mut self) {
        self.visible = false;
    }
}

impl Render for CellDetail {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        if !self.visible {
            return div().into_any_element();
        }

        let display = inspector_text(&self.value);
        let type_label = self.value.type_label();
        let theme = cx.theme();
        let mono = prefs::code_font_family(cx);

        div()
            .absolute()
            .bottom_0()
            .right_0()
            .w(px(320.0))
            .max_h(px(300.0))
            .m_2()
            .bg(theme.popover)
            .border_1()
            .border_color(theme.border)
            .rounded_lg()
            .shadow_lg()
            .overflow_hidden()
            .child(
                h_flex()
                    .px_3()
                    .py_2()
                    .border_b_1()
                    .border_color(theme.border)
                    .child(
                        div()
                            .flex_1()
                            .text_xs()
                            .text_color(theme.muted_foreground)
                            .child(format!("{} — {}", self.column, type_label)),
                    )
                    .child(
                        div()
                            .text_xs()
                            .text_color(theme.muted_foreground)
                            .cursor_pointer()
                            .child("✕")
                            .on_mouse_down(
                                MouseButton::Left,
                                cx.listener(|this, _, _, cx| {
                                    this.hide();
                                    cx.notify();
                                }),
                            ),
                    ),
            )
            .child(
                v_flex().px_3().py_3().flex_1().overflow_hidden().child(
                    div()
                        .font_family(mono)
                        .text_xs()
                        .text_color(theme.foreground)
                        .max_h(px(220.0))
                        .overflow_y_scrollbar()
                        .child(display),
                ),
            )
            .child(
                h_flex()
                    .px_3()
                    .py_2()
                    .border_t_1()
                    .border_color(theme.border)
                    .gap_2()
                    .child(
                        div()
                            .text_xs()
                            .text_color(theme.muted_foreground)
                            .cursor_pointer()
                            .child("Copy value"),
                    ),
            )
            .into_any_element()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn pretty_prints_json() {
        let d = inspector_text(&Value::Json("{\"a\":1,\"b\":2}".into()));
        assert!(d.contains('\n'));
    }

    #[test]
    fn labels_follow_the_value() {
        assert_eq!(Value::Null.type_label(), "NULL");
        assert_eq!(Value::text("null").type_label(), "TEXT");
        assert_eq!(Value::Float(2.5).type_label(), "FLOAT");
        assert_eq!(inspector_text(&Value::text("null")), "null");
    }
}
