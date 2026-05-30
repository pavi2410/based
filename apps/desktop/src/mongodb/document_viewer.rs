// mongodb::document_viewer — paginated find() as a virtual table.

use gpui::{prelude::*, *};
use gpui_component::{
    ActiveTheme,
    button::Button,
    dock::{Panel, PanelEvent},
    h_flex,
    menu::PopupMenu,
    table::{Column, TableState},
    v_flex,
};
use mongodb::Collection;
use mongodb::bson::{Document, doc};
use mongodb::options::FindOptions;

use gpui_component::table::TableEvent;

use crate::widgets::cell_detail::{CellDetail, CellValue, interpret_cell_display};
use crate::widgets::data_table::{configure_row_table, render_row_table};
use crate::widgets::filter_bar::{FilterBar, FilterExpr};
use crate::widgets::virtual_table::{RowDelegate, data_column, replace_table_data};

fn mongo_filter_doc(expr: &FilterExpr) -> Document {
    let s = expr.to_mongo_filter();
    serde_json::from_str::<serde_json::Value>(&s)
        .ok()
        .and_then(|v| mongodb::bson::to_document(&v).ok())
        .unwrap_or_else(|| doc! {})
}

pub struct DocumentViewerPanel {
    focus_handle: FocusHandle,
    collection: Collection<Document>,
    table: Entity<TableState<RowDelegate>>,
    cell_detail: Entity<CellDetail>,
    filter_bar: Entity<FilterBar>,
    limit: i64,
    loading: bool,
    pub(crate) tab_label: SharedString,
}

impl DocumentViewerPanel {
    pub fn new(
        collection: Collection<Document>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Self {
        let delegate = RowDelegate::default();
        let tab_label = collection.name().to_string().into();
        let table = cx.new(|cx| configure_row_table(delegate, window, cx));
        let filter_bar = cx.new(|cx| FilterBar::new(window, cx, vec![]));
        let cell_detail = cx.new(|_| CellDetail::new());
        let mut p = Self {
            focus_handle: cx.focus_handle(),
            collection,
            table,
            cell_detail,
            filter_bar,
            limit: 200,
            loading: false,
            tab_label,
        };
        cx.subscribe(&p.table, |panel, _, event, cx| {
            if let TableEvent::DoubleClickedCell(row_ix, col_ix) = event {
                let row = *row_ix;
                let col = *col_ix;
                let Some((col_name, val)) = panel.cell_snapshot(row, col, cx) else {
                    return;
                };
                panel.cell_detail.update(cx, |d, cx| {
                    d.show(col_name, val);
                    cx.notify();
                });
                cx.notify();
            }
        })
        .detach();
        p.reload(cx);
        p
    }

    fn cell_snapshot(&self, row: usize, col: usize, cx: &App) -> Option<(String, CellValue)> {
        let st = self.table.read(cx);
        let del = st.delegate();
        let col_name = del.columns.get(col)?.key.to_string();
        let txt = del.rows.get(row)?.get(col)?.to_string();
        Some((col_name, interpret_cell_display(&txt)))
    }

    fn reload(&mut self, cx: &mut Context<Self>) {
        self.loading = true;
        let coll = self.collection.clone();
        let lim = self.limit;
        let filter_doc = self
            .filter_bar
            .read(cx)
            .current_expr(cx)
            .map(|e| mongo_filter_doc(&e))
            .unwrap_or_else(|| doc! {});

        cx.spawn(async move |this, cx| {
            let docs = match crate::db::run(cx, async move {
                let opts = FindOptions::builder().limit(lim).build();
                let mut cursor = coll.find(filter_doc, opts).await?;
                let mut docs: Vec<Document> = Vec::new();
                use futures::TryStreamExt;
                while let Some(d) = cursor.try_next().await? {
                    docs.push(d);
                }
                Ok(docs)
            })
            .await
            {
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
                .map(|k| data_column(k.clone(), k.clone()))
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
                    panel.filter_bar.update(cx, |fb, cx| {
                        fb.set_columns_if_empty(keys.clone(), cx);
                    });
                    panel.table.update(cx, |state, cx| {
                        replace_table_data(state, columns, rows, cx);
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

    fn dropdown_menu(
        &mut self,
        menu: PopupMenu,
        _window: &mut Window,
        cx: &mut Context<Self>,
    ) -> PopupMenu {
        crate::based_panel_dropdown!(menu, self, cx)
    }

    crate::based_panel_tab_chrome!();
}

impl Render for DocumentViewerPanel {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let border = cx.theme().border;
        let muted = cx.theme().muted_foreground;
        v_flex()
            .relative()
            .size_full()
            .child(
                h_flex()
                    .p_2()
                    .gap_2()
                    .flex_wrap()
                    .items_center()
                    .border_b_1()
                    .border_color(border)
                    .child(
                        Button::new("mongo-refresh-docs")
                            .label("Refresh")
                            .on_click(cx.listener(|p, _, _, cx| p.reload(cx))),
                    )
                    .child(self.filter_bar.clone())
                    .child(
                        Button::new("mongo-filter-apply")
                            .label("Apply filter")
                            .on_click(cx.listener(|p, _, _, cx| p.reload(cx))),
                    )
                    .child(
                        Button::new("mongo-filter-clear")
                            .label("Clear filter")
                            .on_click(cx.listener(|p, _, window, cx| {
                                p.filter_bar.update(cx, |fb, cx| {
                                    fb.clear(window, cx);
                                });
                                p.reload(cx);
                            })),
                    )
                    .when(self.loading, |h| {
                        h.child(div().text_sm().text_color(muted).child("Loading…"))
                    }),
            )
            .child(render_row_table(&self.table, cx))
            .child(self.cell_detail.clone())
    }
}
