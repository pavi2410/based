// mongodb::pipeline_builder — run aggregation from a JSON pipeline array.

use gpui::{prelude::*, *};
use gpui_component::{
    ActiveTheme,
    button::{Button, ButtonVariants},
    dock::{Panel, PanelEvent},
    h_flex,
    input::{Input, InputState},
    menu::PopupMenu,
    table::{Column, DataTable, TableState},
    v_flex,
};
use mongodb::Collection;
use mongodb::bson::Document;
use time::OffsetDateTime;
use uuid::Uuid;

use crate::connection::ConnectionId;
use crate::query_store::{HistoryEntry, QueryStore, SavedQuery};
use crate::widgets::virtual_table::RowDelegate;

pub struct PipelineBuilderPanel {
    focus_handle: FocusHandle,
    collection: Collection<Document>,
    conn_id: ConnectionId,
    pipeline_json: String,
    result: Entity<TableState<RowDelegate>>,
    status: SharedString,
    save_name_input: Entity<InputState>,
    show_save_prompt: bool,
}

impl PipelineBuilderPanel {
    pub fn new(
        collection: Collection<Document>,
        conn_id: ConnectionId,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Self {
        Self::new_with_pipeline(collection, conn_id, None, window, cx)
    }

    pub fn new_with_pipeline(
        collection: Collection<Document>,
        conn_id: ConnectionId,
        initial_pipeline_json: Option<String>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Self {
        let delegate = RowDelegate::default();
        let result = cx.new(|cx| TableState::new(delegate, window, cx));
        let save_name_input = cx.new(|cx| InputState::new(window, cx));
        let pipeline_json = initial_pipeline_json.unwrap_or_else(|| {
            String::from("[{ \"$match\": {} }, { \"$limit\": 50 }]")
        });
        Self {
            focus_handle: cx.focus_handle(),
            collection,
            conn_id,
            pipeline_json,
            result,
            status: SharedString::from(""),
            save_name_input,
            show_save_prompt: false,
        }
    }

    fn run(&mut self, cx: &mut Context<Self>) {
        let coll = self.collection.clone();
        let raw = self.pipeline_json.clone();
        let conn_id = self.conn_id.clone();
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

            let pipeline_for_history = raw.clone();
            let start = std::time::Instant::now();
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

            let ms = start.elapsed().as_millis() as u64;

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

            let row_count = docs.len() as u64;

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
                    cx.update_global(|store: &mut QueryStore, _| {
                        store.push_history(HistoryEntry {
                            conn_id: conn_id.clone(),
                            query: pipeline_for_history,
                            ran_at: OffsetDateTime::now_utc(),
                            duration_ms: ms,
                            row_count: Some(row_count),
                        });
                    });
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

    fn confirm_save_query(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let name = self
            .save_name_input
            .read(cx)
            .value()
            .trim()
            .to_string();
        if name.is_empty() {
            self.status = "Enter a name to save.".into();
            cx.notify();
            return;
        }

        let pipeline = self.pipeline_json.clone();
        let conn_id = self.conn_id.clone();
        cx.update_global(|store: &mut QueryStore, _| {
            store.save_query(SavedQuery {
                id: format!("q_{}", Uuid::new_v4().as_simple()),
                name,
                connection: conn_id,
                tags: vec![],
                sql: None,
                pipeline: Some(pipeline),
            });
        });

        self.show_save_prompt = false;
        self.save_name_input.update(cx, |state, cx| {
            state.set_value("", window, cx);
        });
        self.status = SharedString::from("Saved to queries.");
        cx.notify();
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
                    .items_center()
                    .child(
                        Button::new("mongo-run-pipe")
                            .primary()
                            .label("Run pipeline")
                            .on_click(cx.listener(|p, _, _, cx| p.run(cx))),
                    )
                    .child(
                        Button::new("mongo-save-pipeline-prompt")
                            .label("Save")
                            .on_click(cx.listener(|p, _, _, cx| {
                                p.show_save_prompt = true;
                                cx.notify();
                            })),
                    )
                    .child(div().flex_1().text_sm().child(self.status.clone())),
            )
            .when(self.show_save_prompt, |v| {
                v.child(
                    h_flex()
                        .px_2()
                        .pb_2()
                        .gap_2()
                        .items_center()
                        .child(
                            div()
                                .flex_1()
                                .min_w(px(160.0))
                                .child(Input::new(&self.save_name_input).cleanable(true)),
                        )
                        .child(
                            Button::new("mongo-save-pipeline-confirm")
                                .primary()
                                .label("Save to queries")
                                .on_click(cx.listener(|p, _, window, cx| {
                                    p.confirm_save_query(window, cx)
                                })),
                        )
                        .child(
                            Button::new("mongo-save-pipeline-cancel").label("Cancel").on_click(
                                cx.listener(|p, _, _, cx| {
                                    p.show_save_prompt = false;
                                    cx.notify();
                                }),
                            ),
                        ),
                )
            })
            .child(
                div()
                    .flex_1()
                    .child(DataTable::new(&self.result).stripe(true).bordered(false)),
            )
    }
}
