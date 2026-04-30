// mongodb::inspector — collection indexes as a table.

use gpui::{prelude::*, *};
use gpui_component::{
    dock::{Panel, PanelEvent},
    v_flex,
    table::{Column, DataTable, TableState},
};
use mongodb::bson::Document;
use mongodb::Collection;

use crate::widgets::virtual_table::RowDelegate;

pub struct CollectionInspectorPanel {
    focus_handle: FocusHandle,
    collection: Collection<Document>,
    table: Entity<TableState<RowDelegate>>,
}

impl CollectionInspectorPanel {
    pub fn new(collection: Collection<Document>, window: &mut Window, cx: &mut Context<Self>) -> Self {
        let delegate = RowDelegate::default();
        let table = cx.new(|cx| TableState::new(delegate, window, cx));
        let mut p = Self {
            focus_handle: cx.focus_handle(),
            collection,
            table,
        };
        p.reload(cx);
        p
    }

    fn reload(&mut self, cx: &mut Context<Self>) {
        let coll = self.collection.clone();
        cx.spawn(async move |this, cx| {
            let mut cursor = match coll.list_indexes(None).await {
                Ok(c) => c,
                Err(e) => {
                    let _ = cx.update(|cx| {
                        this.update(cx, |panel, cx| {
                            panel.table.update(cx, |state, cx| {
                                let d = state.delegate_mut();
                                d.columns =
                                    vec![Column::new("error", "Error")];
                                d.rows =
                                    vec![vec![SharedString::from(e.to_string())]];
                                cx.notify();
                            });
                            cx.notify();
                        })
                    });
                    return;
                }
            };

            let mut rows = Vec::<Vec<String>>::new();
            use futures::TryStreamExt;
            while let Ok(Some(idx)) = cursor.try_next().await {
                let name = idx
                    .options
                    .as_ref()
                    .and_then(|o| o.name.clone())
                    .unwrap_or_else(|| idx.keys.to_string());
                let key_json = idx.keys.to_string();
                let unique = if idx.options.as_ref().and_then(|o| o.unique) == Some(true) {
                    "yes"
                } else {
                    "no"
                };
                rows.push(vec![name, key_json, unique.to_string()]);
            }

            let columns = vec![
                Column::new("name", "Index"),
                Column::new("key", "Key"),
                Column::new("unique", "Unique"),
            ];
            let data: Vec<Vec<SharedString>> = rows
                .into_iter()
                .map(|r| r.into_iter().map(SharedString::from).collect())
                .collect();

            let _ = cx.update(|cx| {
                this.update(cx, |panel, cx| {
                    panel.table.update(cx, |state, cx| {
                        let d = state.delegate_mut();
                        d.columns = columns;
                        d.rows = data;
                        cx.notify();
                    });
                    cx.notify();
                })
            });
        })
        .detach();
    }
}

impl EventEmitter<PanelEvent> for CollectionInspectorPanel {}

impl Focusable for CollectionInspectorPanel {
    fn focus_handle(&self, _: &App) -> FocusHandle {
        self.focus_handle.clone()
    }
}

impl Panel for CollectionInspectorPanel {
    fn panel_name(&self) -> &'static str {
        "MongoCollectionInspector"
    }

    fn closable(&self, _: &App) -> bool {
        true
    }

    fn title(&mut self, _: &mut Window, _cx: &mut Context<Self>) -> impl IntoElement {
        format!("Indexes — {}", self.collection.name())
    }
}

impl Render for CollectionInspectorPanel {
    fn render(&mut self, _window: &mut Window, _cx: &mut Context<Self>) -> impl IntoElement {
        v_flex()
            .size_full()
            .child(DataTable::new(&self.table).stripe(true).bordered(false))
    }
}
