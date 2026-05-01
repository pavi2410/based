//! Floating cell inspector: column, inferred type label, monospace body.

use gpui::{Context, IntoElement, MouseButton, Render, Window, div, prelude::*};
use gpui_component::{ActiveTheme, h_flex, scroll::ScrollableElement, v_flex};

pub enum CellValue {
    Text(String),
    Integer(i64),
    Float(f64),
    Boolean(bool),
    Json(String), // raw JSON string — will be pretty-printed
    Null,
    Blob(usize), // byte count only
}

impl CellValue {
    pub fn type_label(&self) -> &'static str {
        match self {
            Self::Text(_) => "TEXT",
            Self::Integer(_) => "INTEGER",
            Self::Float(_) => "FLOAT",
            Self::Boolean(_) => "BOOLEAN",
            Self::Json(_) => "JSON",
            Self::Null => "NULL",
            Self::Blob(_) => "BLOB",
        }
    }

    pub fn display(&self) -> String {
        match self {
            Self::Text(s) => s.clone(),
            Self::Integer(n) => n.to_string(),
            Self::Float(f) => format!("{f:.6}"),
            Self::Boolean(b) => b.to_string(),
            Self::Json(s) => pretty_json(s),
            Self::Null => "NULL".to_string(),
            Self::Blob(n) => format!("<{n} bytes>"),
        }
    }
}

fn pretty_json(s: &str) -> String {
    serde_json::from_str::<serde_json::Value>(s)
        .ok()
        .and_then(|v| serde_json::to_string_pretty(&v).ok())
        .unwrap_or_else(|| s.to_string())
}

pub fn interpret_cell_display(s: &str) -> CellValue {
    let t = s.trim();
    if t.is_empty() {
        return CellValue::Null;
    }
    if t.eq_ignore_ascii_case("null") {
        return CellValue::Null;
    }
    if let Ok(n) = t.parse::<i64>() {
        return CellValue::Integer(n);
    }
    if let Ok(n) = t.parse::<f64>() {
        return CellValue::Float(n);
    }
    if let Ok(b) = t.parse::<bool>() {
        return CellValue::Boolean(b);
    }
    if (t.starts_with('{') && t.ends_with('}')) || (t.starts_with('[') && t.ends_with(']')) {
        return CellValue::Json(t.to_string());
    }
    CellValue::Text(t.to_string())
}

pub struct CellDetail {
    pub column: String,
    pub value: CellValue,
    pub visible: bool,
}

impl CellDetail {
    pub fn new() -> Self {
        Self {
            column: String::new(),
            value: CellValue::Null,
            visible: false,
        }
    }

    pub fn show(&mut self, column: String, value: CellValue) {
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

        let display = self.value.display();
        let type_label = self.value.type_label();
        let theme = cx.theme();
        let mono = theme.mono_font_family.clone();

        div()
            .absolute()
            .bottom_0()
            .right_0()
            .w(gpui::px(320.0))
            .max_h(gpui::px(300.0))
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
                        .max_h(gpui::px(220.0))
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
        let v = CellValue::Json("{\"a\":1,\"b\":2}".into());
        let d = v.display();
        assert!(d.contains('\n'));
    }

    #[test]
    fn null_displays() {
        assert_eq!(CellValue::Null.display(), "NULL");
        assert_eq!(CellValue::Null.type_label(), "NULL");
    }

    #[test]
    fn blob_shows_byte_count() {
        let d = CellValue::Blob(1024).display();
        assert_eq!(d, "<1024 bytes>");
    }
}
