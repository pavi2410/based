// mongodb::inspector — collection stats (collStats) and indexes.

use gpui::{prelude::*, *};
use gpui_component::{
    ActiveTheme,
    button::Button,
    dock::{Panel, PanelEvent},
    h_flex,
    menu::PopupMenu,
    table::TableState,
    v_flex,
};
use mongodb::Collection;
use mongodb::bson::{Document, doc};

use crate::widgets::compact_description_list_vertical;
use crate::widgets::data_table::{configure_row_table, render_row_table};
use crate::widgets::panel::tab_button_styled;
use crate::widgets::virtual_table::{
    RowDelegate, data_column, empty_column_meta, replace_table_data,
};

#[derive(Clone, Copy, PartialEq, Eq, Default)]
enum MongoInspectorTab {
    #[default]
    Stats,
    Indexes,
}

pub struct CollectionInspectorPanel {
    focus_handle: FocusHandle,
    collection: Collection<Document>,
    stats_rows: Vec<(SharedString, SharedString)>,
    indexes_tbl: Entity<TableState<RowDelegate>>,
    tab: MongoInspectorTab,
    pub(crate) tab_label: SharedString,
}

impl CollectionInspectorPanel {
    pub fn new(
        collection: Collection<Document>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Self {
        let idx_del = RowDelegate::default();
        let indexes_tbl = cx.new(|cx| configure_row_table(idx_del, window, cx));
        let tab_label = format!("{} (schema)", collection.name()).into();
        let mut p = Self {
            focus_handle: cx.focus_handle(),
            collection,
            stats_rows: vec![],
            indexes_tbl,
            tab: MongoInspectorTab::default(),
            tab_label,
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
                            panel.stats_rows =
                                vec![("Error".into(), SharedString::from(msg.clone()))];
                            panel.indexes_tbl.update(cx, |state, cx| {
                                replace_table_data(
                                    state,
                                    vec![data_column("error", "Error")],
                                    vec![vec![SharedString::from(msg)]],
                                    empty_column_meta(1),
                                    cx,
                                );
                            });
                            cx.notify();
                        })
                    });
                }
                Ok((stat_rows, ix_rows)) => {
                    let stats_rows: Vec<(SharedString, SharedString)> = stat_rows
                        .into_iter()
                        .map(|r| {
                            let label = r.first().cloned().unwrap_or_default();
                            let value = r.get(1).cloned().unwrap_or_default();
                            (label.into(), value.into())
                        })
                        .collect();

                    let ix_columns = vec![
                        data_column("name", "Index"),
                        data_column("key", "Key"),
                        data_column("unique", "Unique"),
                    ];
                    let ix_data: Vec<Vec<SharedString>> = ix_rows
                        .into_iter()
                        .map(|r| r.into_iter().map(SharedString::from).collect())
                        .collect();

                    let _ = cx.update(|cx| {
                        this.update(cx, |panel, cx| {
                            panel.stats_rows = stats_rows;
                            let ix_meta = empty_column_meta(ix_columns.len());
                            panel.indexes_tbl.update(cx, |state, cx| {
                                replace_table_data(state, ix_columns, ix_data, ix_meta, cx);
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
        tab_button_styled(id, label, self.tab == tab).on_click(cx.listener(
            move |panel, _, _, cx| {
                panel.tab = tab;
                cx.notify();
            },
        ))
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

    crate::based_panel_tab_chrome!();
}

impl Render for CollectionInspectorPanel {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let border = cx.theme().border;
        let stats_rows = self.stats_rows.clone();

        let body: AnyElement = match self.tab {
            MongoInspectorTab::Stats => div()
                .p_3()
                .child(compact_description_list_vertical(stats_rows, false))
                .into_any_element(),
            MongoInspectorTab::Indexes => {
                render_row_table(&self.indexes_tbl, cx).into_any_element()
            }
        };

        v_flex()
            .size_full()
            .gap_2()
            .child(
                h_flex()
                    .w_full()
                    .justify_center()
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
                    .child(body),
            )
    }
}
