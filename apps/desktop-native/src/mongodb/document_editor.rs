// mongodb::document_editor — replace / patch document by _id (JSON).

use gpui::{prelude::*, *};
use gpui_component::{
    ActiveTheme,
    button::{Button, ButtonVariants},
    dock::{Panel, PanelEvent},
    h_flex, v_flex,
};
use mongodb::bson::Document;
use mongodb::Collection;

use crate::mongodb::mutations::{document_from_json, replace_by_id};

pub struct DocumentEditorPanel {
    focus_handle: FocusHandle,
    collection: Collection<Document>,
    id: String,
    body: String,
    status: SharedString,
}

impl DocumentEditorPanel {
    pub fn new(collection: Collection<Document>, _window: &mut Window, cx: &mut Context<Self>) -> Self {
        Self {
            focus_handle: cx.focus_handle(),
            collection,
            id: String::new(),
            body: String::from("{ }"),
            status: SharedString::from(""),
        }
    }

    fn save_replace(&mut self, cx: &mut Context<Self>) {
        let id = self.id.clone();
        let body = self.body.clone();
        let coll = self.collection.clone();
        cx.spawn(async move |this, cx| {
            let doc = match document_from_json(&body) {
                Ok(d) => d,
                Err(e) => {
                    let _ = cx.update(|cx| {
                        this.update(cx, |p, cx| {
                            p.status = format!("parse error: {e}").into();
                            cx.notify();
                        })
                    });
                    return;
                }
            };
            let r = crate::db::run(cx, async move { replace_by_id(&coll, &id, doc).await }).await;
            let _ = cx.update(|cx| {
                this.update(cx, |p, cx| {
                    p.status = match r {
                        Ok(n) => format!("replaced, modified: {n}").into(),
                        Err(e) => format!("error: {e}").into(),
                    };
                    cx.notify();
                })
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

    fn closable(&self, _: &App) -> bool {
        true
    }

    fn title(&mut self, _: &mut Window, _cx: &mut Context<Self>) -> impl IntoElement {
        format!("Edit — {}", self.collection.name())
    }
}

impl Render for DocumentEditorPanel {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let border = cx.theme().border;
        let id_disp: SharedString = self.id.clone().into();
        let body_disp: SharedString = self.body.clone().into();

        v_flex()
            .size_full()
            .gap_2()
            .p_2()
            .child(div().text_xs().text_color(cx.theme().muted_foreground).child("_id"))
            .child(
                div()
                    .p_2()
                    .border_1()
                    .border_color(border)
                    .font_family("monospace")
                    .text_sm()
                    .child(id_disp),
            )
            .child(div().text_xs().text_color(cx.theme().muted_foreground).child("Document JSON"))
            .child(
                div()
                    .flex_1()
                    .min_h(px(160.0))
                    .p_2()
                    .border_1()
                    .border_color(border)
                    .font_family("monospace")
                    .text_xs()
                    .child(body_disp),
            )
            .child(
                h_flex()
                    .gap_2()
                    .child(
                        Button::new("mongo-save-doc")
                            .primary()
                            .label("Replace document")
                            .on_click(cx.listener(|p, _, _, cx| p.save_replace(cx))),
                    )
                    .child(div().text_sm().child(self.status.clone())),
            )
    }
}
