// mongodb::change_stream — samples up to 64 change events (requires replica set / CSRS).

use gpui::{prelude::*, *};
use gpui_component::{
    button::{Button, ButtonVariants},
    Disableable,
    dock::{Panel, PanelEvent},
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
            let mut stream = match coll.watch(None, None).await {
                Ok(s) => s,
                Err(e) => {
                    cx.update(|cx| {
                        this.update(cx, |p, cx| {
                            p.busy = false;
                            p.lines = vec![format!("watch() failed: {e}")];
                            cx.notify();
                        })
                    })
                    .ok();
                    return;
                }
            };

            use futures::StreamExt;
            let mut count = 0usize;
            while let Some(evt) = stream.next().await {
                match evt {
                    Ok(change) => {
                        count += 1;
                        let line = format!("{change:?}");
                        let _ = cx.update(|cx| {
                            this.update(cx, |p, cx| {
                                p.lines.push(line);
                                cx.notify();
                            })
                        });
                        if count >= 64 {
                            break;
                        }
                    }
                    Err(e) => {
                        let _ = cx.update(|cx| {
                            this.update(cx, |p, cx| {
                                p.lines.push(format!("stream error: {e}"));
                                p.busy = false;
                                cx.notify();
                            })
                        });
                        return;
                    }
                }
            }

            let _ = cx.update(|cx| {
                this.update(cx, |p, cx| {
                    p.busy = false;
                    if p.lines.len() == 1 {
                        p.lines.push("(stream ended)".into());
                    }
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
