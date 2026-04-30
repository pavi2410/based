// mongodb::change_stream — samples up to 64 change events (requires replica set / CSRS).

use gpui::{prelude::*, *};
use gpui_component::{
    button::{Button, ButtonVariants},
    Disableable,
    dock::{Panel, PanelEvent},
    menu::PopupMenu,
    h_flex, v_flex,
};
use mongodb::bson::Document;
use mongodb::Collection;

pub struct ChangeStreamPanel {
    focus_handle: FocusHandle,
    collection: Collection<Document>,
    lines: Vec<String>,
    busy: bool,
}

impl ChangeStreamPanel {
    pub fn new(collection: Collection<Document>, _window: &mut Window, cx: &mut Context<Self>) -> Self {
        Self {
            focus_handle: cx.focus_handle(),
            collection,
            lines: vec![(
                "Replica-set deployments support $changeStream. \
                 Click “Sample stream” to read up to 64 events or an error."
            )
                .into()],
            busy: false,
        }
    }

    fn sample(&mut self, cx: &mut Context<Self>) {
        if self.busy {
            return;
        }
        self.busy = true;
        self.lines = vec!["Opening change stream…".into()];
        let coll = self.collection.clone();
        cx.spawn(async move |this, cx| {
            let result = crate::db::run(cx, async move {
                use futures::StreamExt;
                let mut stream = coll
                    .watch(None, None)
                    .await
                    .map_err(|e| anyhow::anyhow!("watch() failed: {e}"))?;
                let mut out_lines = Vec::<String>::new();
                let mut count = 0usize;
                loop {
                    match stream.next().await {
                        None => break,
                        Some(Ok(change)) => {
                            out_lines.push(format!("{change:?}"));
                            count += 1;
                            if count >= 64 {
                                break;
                            }
                        }
                        Some(Err(e)) => {
                            return Err(anyhow::anyhow!("stream error: {e}"));
                        }
                    }
                }
                Ok(out_lines)
            })
            .await;

            let _ = cx.update(|cx| {
                this.update(cx, |p, cx| {
                    match result {
                        Ok(extra) => {
                            p.lines.extend(extra);
                            if p.lines.len() == 1 {
                                p.lines.push("(stream ended)".into());
                            }
                        }
                        Err(msg) => {
                            p.lines = vec![msg.to_string()];
                        }
                    }
                    p.busy = false;
                    cx.notify();
                })
            });
        })
        .detach();
    }
}

impl EventEmitter<PanelEvent> for ChangeStreamPanel {}

impl Focusable for ChangeStreamPanel {
    fn focus_handle(&self, _: &App) -> FocusHandle {
        self.focus_handle.clone()
    }
}

impl Panel for ChangeStreamPanel {
    fn panel_name(&self) -> &'static str {
        "MongoChangeStream"
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
        format!("Changes — {}", self.collection.name())
    }
}

impl Render for ChangeStreamPanel {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let text: SharedString = self.lines.join("\n").into();
        v_flex()
            .size_full()
            .child(
                h_flex()
                    .p_2()
                    .gap_2()
                    .child(
                        Button::new("mongo-watch-sample")
                            .primary()
                            .label("Sample stream")
                            .disabled(self.busy)
                            .on_click(cx.listener(|p, _, _, cx| p.sample(cx))),
                    ),
            )
            .child(
                div()
                    .id("mongo-change-scroll")
                    .flex_1()
                    .overflow_y_scroll()
                    .p_2()
                    .font_family("monospace")
                    .text_xs()
                    .child(text),
            )
    }
}
