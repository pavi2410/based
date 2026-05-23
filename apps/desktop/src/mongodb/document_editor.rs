// mongodb::document_editor — insert / replace documents via JSON (multiline editor).

use gpui::{prelude::*, *};
use gpui_component::{
    ActiveTheme,
    button::{Button, ButtonVariants},
    dock::{Panel, PanelEvent},
    h_flex,
    input::{Input, InputState},
    menu::PopupMenu,
    v_flex,
};
use mongodb::Collection;
use mongodb::bson::Document;
use mongodb::bson::doc;

use crate::mongodb::mutations::document_from_json;
use crate::widgets::sql_editor::new_json_input;

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum EditorMode {
    Insert,
    Edit,
}

fn document_to_pretty_json(doc: &Document) -> String {
    serde_json::to_value(doc)
        .ok()
        .and_then(|v| serde_json::to_string_pretty(&v).ok())
        .unwrap_or_else(|| "{\n  \n}".to_string())
}

pub struct DocumentEditorPanel {
    focus_handle: FocusHandle,
    collection: Collection<Document>,
    mode: EditorMode,
    json_input: Entity<InputState>,
    /// When editing, the loaded document (used for `_id` on replace).
    original: Option<Document>,
    error: Option<String>,
    status: SharedString,
}

impl DocumentEditorPanel {
    pub fn new_insert(
        collection: Collection<Document>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Self {
        let json_input = new_json_input("{\n  \n}", window, cx);
        let panel = Self {
            focus_handle: cx.focus_handle(),
            collection,
            mode: EditorMode::Insert,
            json_input,
            original: None,
            error: None,
            status: SharedString::default(),
        };
        panel
    }

    pub fn new_edit(
        collection: Collection<Document>,
        doc: Document,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Self {
        let pretty = document_to_pretty_json(&doc);
        let json_input = new_json_input(&pretty, window, cx);
        let panel = Self {
            focus_handle: cx.focus_handle(),
            collection,
            mode: EditorMode::Edit,
            json_input,
            original: Some(doc),
            error: None,
            status: SharedString::default(),
        };
        panel
    }

    /// Kept for callers that open an editor without a pre-loaded document (same as insert).
    pub fn new(
        collection: Collection<Document>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Self {
        Self::new_insert(collection, window, cx)
    }

    fn save(&mut self, _window: &mut Window, cx: &mut Context<Self>) {
        let text = self.json_input.read(cx).value().to_string();
        let mut doc = match document_from_json(&text) {
            Ok(d) => d,
            Err(e) => {
                self.error = Some(e.to_string());
                self.status = SharedString::default();
                cx.notify();
                return;
            }
        };

        self.error = None;
        self.status = SharedString::from("Saving…");
        cx.notify();

        let coll = self.collection.clone();
        let mode = self.mode;
        let original = self.original.clone();

        cx.spawn(async move |this, cx| {
            let outcome = crate::db::run(cx, async move {
                match mode {
                    EditorMode::Insert => {
                        coll.insert_one(doc, None)
                            .await
                            .map_err(|e| anyhow::anyhow!(e))?;
                    }
                    EditorMode::Edit => {
                        let id = original
                            .as_ref()
                            .and_then(|o| o.get("_id"))
                            .cloned()
                            .ok_or_else(|| anyhow::anyhow!("missing _id on original document"))?;
                        doc.insert("_id", id.clone());
                        coll.replace_one(doc! { "_id": id }, doc, None)
                            .await
                            .map_err(|e| anyhow::anyhow!(e))?;
                    }
                }
                Ok::<(), anyhow::Error>(())
            })
            .await;

            let _ = this.update(cx, |panel, cx| {
                match outcome {
                    Ok(()) => {
                        panel.status = SharedString::from("Saved.");
                        panel.error = None;
                    }
                    Err(e) => {
                        panel.status = SharedString::default();
                        panel.error = Some(format!("{e:#}"));
                    }
                }
                cx.notify();
            });
        })
        .detach();
    }
}

impl EventEmitter<PanelEvent> for DocumentEditorPanel {}

impl Focusable for DocumentEditorPanel {
    fn focus_handle(&self, _: &App) -> FocusHandle {
        self.focus_handle.clone()
    }
}

impl Panel for DocumentEditorPanel {
    fn panel_name(&self) -> &'static str {
        "MongoDocumentEditor"
    }

    fn dropdown_menu(
        &mut self,
        menu: PopupMenu,
        _window: &mut Window,
        cx: &mut Context<Self>,
    ) -> PopupMenu {
        crate::based_panel_dropdown!(menu, self, cx)
    }

    fn closable(&self, _: &App) -> bool {
        true
    }

    fn title(&mut self, _: &mut Window, _cx: &mut Context<Self>) -> impl IntoElement {
        match self.mode {
            EditorMode::Insert => format!("Insert — {}", self.collection.name()),
            EditorMode::Edit => format!("Edit — {}", self.collection.name()),
        }
    }
}

impl Render for DocumentEditorPanel {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let border = cx.theme().border;
        let danger = cx.theme().danger;
        let mode_label = match self.mode {
            EditorMode::Insert => "Insert document",
            EditorMode::Edit => "Edit document",
        };

        let error_row = self.error.as_ref().map(|err| {
            div()
                .px_3()
                .py_2()
                .text_xs()
                .text_color(danger)
                .child(err.clone())
        });

        v_flex()
            .size_full()
            .gap_2()
            .child(
                h_flex()
                    .p_2()
                    .gap_2()
                    .items_center()
                    .border_b_1()
                    .border_color(border)
                    .child(
                        div()
                            .flex_1()
                            .text_sm()
                            .text_color(cx.theme().muted_foreground)
                            .child(mode_label),
                    )
                    .child(
                        Button::new("mongo-save-doc")
                            .primary()
                            .label("Save")
                            .on_click(cx.listener(|p, _, window, cx| p.save(window, cx))),
                    ),
            )
            .when_some(error_row, |v, row| v.child(row))
            .when(!self.status.is_empty(), |v| {
                v.child(
                    div()
                        .px_3()
                        .text_xs()
                        .text_color(cx.theme().muted_foreground)
                        .child(self.status.clone()),
                )
            })
            .child(
                div()
                    .flex_1()
                    .min_h(px(200.0))
                    .p_2()
                    .border_1()
                    .border_color(border)
                    .font_family("monospace")
                    .text_sm()
                    .child(Input::new(&self.json_input).h_full().cleanable(false)),
            )
    }
}

#[cfg(test)]
mod tests {
    use crate::mongodb::mutations::document_from_json;

    #[test]
    fn valid_json_object_passes() {
        assert!(document_from_json(r#"{"a": 1}"#).is_ok());
    }

    #[test]
    fn invalid_json_fails() {
        assert!(document_from_json("{bad json}").is_err());
    }

    #[test]
    fn json_array_fails() {
        assert!(document_from_json("[1,2,3]").is_err());
    }
}
