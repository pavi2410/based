// mongodb::pipeline_builder — run aggregation from a JSON pipeline array.

use gpui::{prelude::*, *};
use gpui_component::{
    ActiveTheme,
    button::{Button, ButtonVariants},
    dock::{Panel, PanelEvent},
    h_flex,
    menu::PopupMenu,
    table::{Column, DataTable, TableState},
    v_flex,
};
use mongodb::Collection;
use mongodb::bson::Document;

use crate::widgets::virtual_table::RowDelegate;

pub struct PipelineBuilderPanel {
    focus_handle: FocusHandle,
    collection: Collection<Document>,
    pipeline_json: String,
    result: Entity<TableState<RowDelegate>>,
    status: SharedString,
}

impl PipelineBuilderPanel {
    pub fn new(
        collection: Collection<Document>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Self {
        let delegate = RowDelegate::default();
        let result = cx.new(|cx| TableState::new(delegate, window, cx));
        Self {
            focus_handle: cx.focus_handle(),
            collection,
            pipeline_json: String::from("[{ \"$match\": {} }, { \"$limit\": 50 }]"),
            result,
            status: SharedString::from(""),
        }
    }

    fn run(&mut self, cx: &mut Context<Self>) {
        let coll = self.collection.clone();
        let raw = self.pipeline_json.clone();
        cx.spawn(async move |this, cx| {
            let vals: Vec<serde_json::Value> = match serde_json::from_str(&raw) {
                Ok(v) => v,
                Err(e) => {
                    let _ = cx.update(|cx| {
                        this.update(cx, |p, cx| {
                            p.status = format!("invalid JSON: {e}").into();
                            cx.notify();
                        })
                    });
                    return;
                }
            };

            let mut stages = Vec::<Document>::new();
            for v in vals {
                match mongodb::bson::to_document(&v) {
                    Ok(d) => stages.push(d),
                    Err(e) => {
                        let _ = cx.update(|cx| {
                            this.update(cx, |p, cx| {
                                p.status = format!("BSON error: {e}").into();
                                cx.notify();
                            })
                        });
                        return;
                    }
                }
            }

            let docs_result = crate::db::run(cx, async move {
                let mut cursor = coll.aggregate(stages, None).await?;
                let mut docs = Vec::<Document>::new();
                use futures::TryStreamExt;
                while let Some(d) = cursor.try_next().await.unwrap_or(None) {
                    docs.push(d);
                }
                Ok(docs)
            })
            .await;

            let docs = match docs_result {
                Ok(d) => d,
                Err(e) => {
                    let _ = cx.update(|cx| {
                        this.update(cx, |p, cx| {
                            p.status = format!("aggregate error: {e}").into();
                            cx.notify();
                        })
                    });
                    return;
                }
            };

            let mut keys: Vec<String> = Vec::new();
            for d in &docs {
                for k in d.keys() {
                    if !keys.contains(k) {
                        keys.push(k.clone());
                    }
                }
            }

            let columns: Vec<Column> = keys
                .iter()
                .map(|k| Column::new(k.clone(), k.clone()))
                .collect();
            let rows: Vec<Vec<SharedString>> = docs
                .iter()
                .map(|d| {
                    keys.iter()
                        .map(|k| {
                            d.get(k)
                                .map(|v| SharedString::from(v.to_string()))
                                .unwrap_or_default()
                        })
                        .collect()
                })
                .collect();

            let _ = cx.update(|cx| {
                this.update(cx, |panel, cx| {
                    panel.status = format!("{} rows", rows.len()).into();
                    panel.result.update(cx, |state, cx| {
                        let d = state.delegate_mut();
                        d.columns = columns;
                        d.rows = rows;
                        cx.notify();
                    });
                    cx.notify();
                })
            });
        })
        .detach();
    }
}

impl EventEmitter<PanelEvent> for PipelineBuilderPanel {}

impl Focusable for PipelineBuilderPanel {
    fn focus_handle(&self, _: &App) -> FocusHandle {
        self.focus_handle.clone()
    }
}

impl Panel for PipelineBuilderPanel {
    fn panel_name(&self) -> &'static str {
        "MongoPipelineBuilder"
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
        format!("Pipeline — {}", self.collection.name())
    }
}

impl Render for PipelineBuilderPanel {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let border = cx.theme().border;
        let txt: SharedString = self.pipeline_json.clone().into();
        v_flex()
            .size_full()
            .child(
                div()
                    .flex_1()
                    .min_h(px(120.0))
                    .p_2()
                    .border_1()
                    .border_color(border)
                    .font_family("monospace")
                    .text_xs()
                    .child(txt),
            )
            .child(
                h_flex()
                    .p_2()
                    .gap_2()
                    .child(
                        Button::new("mongo-run-pipe")
                            .primary()
                            .label("Run pipeline")
                            .on_click(cx.listener(|p, _, _, cx| p.run(cx))),
                    )
                    .child(div().text_sm().child(self.status.clone())),
            )
            .child(
                div()
                    .flex_1()
                    .child(DataTable::new(&self.result).stripe(true).bordered(false)),
            )
    }
}
