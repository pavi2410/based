// mongodb::inspector — collection stats (collStats) and indexes.

use gpui::{prelude::*, *};
use gpui_component::{
    ActiveTheme, Sizable,
    button::{Button, ButtonVariants},
    dock::{Panel, PanelEvent},
    h_flex,
    menu::PopupMenu,
    table::{Column, TableState},
    v_flex,
};
use mongodb::Collection;
use mongodb::bson::{Document, doc};

use crate::widgets::data_table::read_only_striped;
use crate::widgets::virtual_table::RowDelegate;

#[derive(Clone, Copy, PartialEq, Eq, Default)]
enum MongoInspectorTab {
    #[default]
    Stats,
    Indexes,
}

pub struct CollectionInspectorPanel {
    focus_handle: FocusHandle,
    collection: Collection<Document>,
    stats_tbl: Entity<TableState<RowDelegate>>,
    indexes_tbl: Entity<TableState<RowDelegate>>,
    tab: MongoInspectorTab,
}

impl CollectionInspectorPanel {
    pub fn new(
        collection: Collection<Document>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Self {
        let stats_del = RowDelegate::default();
        let stats_tbl = cx.new(|cx| TableState::new(stats_del, window, cx));
        let idx_del = RowDelegate::default();
        let indexes_tbl = cx.new(|cx| TableState::new(idx_del, window, cx));
        let mut p = Self {
            focus_handle: cx.focus_handle(),
            collection,
            stats_tbl,
            indexes_tbl,
            tab: MongoInspectorTab::default(),
        };
        p.reload(cx);
        p
    }

    fn reload(&mut self, cx: &mut Context<Self>) {
        let coll = self.collection.clone();
        cx.spawn(async move |this, cx| {
            let outcome = crate::db::run(cx, async move {
                let ns = coll.namespace();
                let stats_doc = coll
                    .client()
                    .database(&ns.db)
                    .run_command(doc! { "collStats": &ns.coll, "scale": 1_i32 }, None)
                    .await
                    .ok();

                let mut stat_rows: Vec<Vec<String>> = Vec::new();
                if let Some(ref doc) = stats_doc {
                    let pairs: [(&str, &str); 6] = [
                        ("Documents", "count"),
                        ("Avg document size (bytes)", "avgObjSize"),
                        ("Data size (bytes)", "size"),
                        ("Storage size (bytes)", "storageSize"),
                        ("Total index size (bytes)", "totalIndexSize"),
                        ("Indexes", "nindexes"),
                    ];
                    for (label, key) in pairs {
                        if let Some(v) = doc.get(key) {
                            stat_rows.push(vec![label.to_string(), v.to_string()]);
                        }
                    }
                }
                if stat_rows.is_empty() {
                    stat_rows.push(vec![
                        "(no stats)".into(),
                        "collStats unavailable or empty".into(),
                    ]);
                }

                let mut cursor = coll.list_indexes(None).await?;
                let mut ix_rows = Vec::<Vec<String>>::new();
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
                    ix_rows.push(vec![name, key_json, unique.to_string()]);
                }

                Ok((stat_rows, ix_rows))
            })
            .await;

            match outcome {
                Err(e) => {
                    let msg = e.to_string();
                    let _ = cx.update(|cx| {
                        this.update(cx, |panel, cx| {
                            panel.stats_tbl.update(cx, |state, cx| {
                                let d = state.delegate_mut();
                                d.columns = vec![Column::new("error", "Error")];
                                d.rows = vec![vec![SharedString::from(msg.clone())]];
                                cx.notify();
                            });
                            panel.indexes_tbl.update(cx, |state, cx| {
                                let d = state.delegate_mut();
                                d.columns = vec![Column::new("error", "Error")];
                                d.rows = vec![vec![SharedString::from(msg)]];
                                cx.notify();
                            });
                            cx.notify();
                        })
                    });
                }
                Ok((stat_rows, ix_rows)) => {
                    let st_columns = vec![
                        Column::new("metric", "Metric"),
                        Column::new("value", "Value"),
                    ];
                    let st_data: Vec<Vec<SharedString>> = stat_rows
                        .into_iter()
                        .map(|r| r.into_iter().map(SharedString::from).collect())
                        .collect();

                    let ix_columns = vec![
                        Column::new("name", "Index"),
                        Column::new("key", "Key"),
                        Column::new("unique", "Unique"),
                    ];
                    let ix_data: Vec<Vec<SharedString>> = ix_rows
                        .into_iter()
                        .map(|r| r.into_iter().map(SharedString::from).collect())
                        .collect();

                    let _ = cx.update(|cx| {
                        this.update(cx, |panel, cx| {
                            panel.stats_tbl.update(cx, |state, cx| {
                                let d = state.delegate_mut();
                                d.columns = st_columns;
                                d.rows = st_data;
                                cx.notify();
                            });
                            panel.indexes_tbl.update(cx, |state, cx| {
                                let d = state.delegate_mut();
                                d.columns = ix_columns;
                                d.rows = ix_data;
                                cx.notify();
                            });
                            cx.notify();
                        })
                    });
                }
            }
        })
        .detach();
    }

    fn tab_button(
        &self,
        id: &'static str,
        label: &'static str,
        tab: MongoInspectorTab,
        cx: &mut Context<Self>,
    ) -> Button {
        let active = self.tab == tab;
        let mut b = Button::new(id).label(label).small();
        if active {
            b = b.outline();
        } else {
            b = b.ghost();
        }
        b.on_click(cx.listener(move |panel, _, _, cx| {
            panel.tab = tab;
            cx.notify();
        }))
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
        format!("Inspector — {}", self.collection.name())
    }
}

impl Render for CollectionInspectorPanel {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let border = cx.theme().border;
        let active_tbl = match self.tab {
            MongoInspectorTab::Stats => &self.stats_tbl,
            MongoInspectorTab::Indexes => &self.indexes_tbl,
        };

        v_flex()
            .size_full()
            .gap_2()
            .child(
                h_flex()
                    .gap_2()
                    .py_2()
                    .px_2()
                    .border_b_1()
                    .border_color(border)
                    .child(self.tab_button(
                        "mongo-insp-stats",
                        "Stats",
                        MongoInspectorTab::Stats,
                        cx,
                    ))
                    .child(self.tab_button(
                        "mongo-insp-indexes",
                        "Indexes",
                        MongoInspectorTab::Indexes,
                        cx,
                    )),
            )
            .child(
                div()
                    .flex_1()
                    .min_h(px(200.0))
                    .border_1()
                    .border_color(border)
                    .child(read_only_striped(active_tbl)),
            )
    }
}
