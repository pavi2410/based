// mongodb::document_viewer — paginated find() as a virtual table.

use gpui::{prelude::*, *};
use gpui_component::{
    ActiveTheme,
    button::Button,
    dock::{Panel, PanelEvent},
    h_flex, v_flex,
    table::{Column, DataTable, TableState},
};
use mongodb::bson::{doc, Document};
use mongodb::options::FindOptions;
use mongodb::Collection;

use crate::widgets::virtual_table::RowDelegate;

pub struct DocumentViewerPanel {
    focus_handle: FocusHandle,
    collection: Collection<Document>,
    table: Entity<TableState<RowDelegate>>,
    limit: i64,
    loading: bool,
}

impl DocumentViewerPanel {
    pub fn new(
        collection: Collection<Document>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Self {
        let delegate = RowDelegate::default();
        let table = cx.new(|cx| {
            TableState::new(delegate, window, cx)
                .row_selectable(true)
                .cell_selectable(true)
        });
        let mut p = Self {
            focus_handle: cx.focus_handle(),
            collection,
            table,
            limit: 200,
            loading: false,
        };
        p.reload(cx);
        p
    }

    fn reload(&mut self, cx: &mut Context<Self>) {
        self.loading = true;
        let coll = self.collection.clone();
        let lim = self.limit;
        cx.spawn(async move |this, cx| {
            let docs = crate::tokio_bridge::block_on_db(async move {
                let opts = FindOptions::builder().limit(lim).build();
                let mut cursor = coll.find(doc! {}, opts).await?;
                let mut docs: Vec<Document> = Vec::new();
                use futures::TryStreamExt;
                while let Some(d) = cursor.try_next().await? {
                    docs.push(d);
                }
                Ok::<_, mongodb::error::Error>(docs)
            });

            let docs = match docs {
                Ok(d) => d,
                Err(_) => {
                    let _ = cx.update(|cx| {
                        this.update(cx, |panel, cx| {
                            panel.loading = false;
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
            if !keys.iter().any(|k| k == "_id") {
                keys.insert(0, "_id".into());
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
                    panel.loading = false;
                    panel.table.update(cx, |state, cx| {
                        let del = state.delegate_mut();
                        del.columns = columns;
                        del.rows = rows;
                        cx.notify();
                    });
                    cx.notify();
                })
            });
        })
        .detach();
    }
}

impl EventEmitter<PanelEvent> for DocumentViewerPanel {}

impl Focusable for DocumentViewerPanel {
    fn focus_handle(&self, _: &App) -> FocusHandle {
        self.focus_handle.clone()
    }
}

impl Panel for DocumentViewerPanel {
    fn panel_name(&self) -> &'static str {
        "MongoDocumentViewer"
    }

    fn closable(&self, _: &App) -> bool {
        true
    }

    fn title(&mut self, _: &mut Window, _cx: &mut Context<Self>) -> impl IntoElement {
        self.collection.name().to_string()
    }
}

impl Render for DocumentViewerPanel {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let border = cx.theme().border;
        let muted = cx.theme().muted_foreground;
        v_flex()
            .size_full()
            .child(
                h_flex()
                    .p_2()
                    .gap_2()
                    .border_b_1()
                    .border_color(border)
                    .child(
                        Button::new("mongo-refresh-docs")
                            .label("Refresh")
                            .on_click(cx.listener(|p, _, _, cx| p.reload(cx))),
                    )
                    .when(self.loading, |h| {
                        h.child(div().text_sm().text_color(muted).child("Loading…"))
                    }),
            )
            .child(DataTable::new(&self.table).stripe(true).bordered(false))
    }
}
