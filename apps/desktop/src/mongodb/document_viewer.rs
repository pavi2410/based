// mongodb::document_viewer — paginated find() as a virtual table.

use based_core::Value;
use based_core::{ColumnCatalog, ResultColumn};
use based_mongo::documents_to_result_set;
use gpui_kit::assets::IconName as AssetIcon;

use gpui_kit::component::{
    ActiveTheme, Icon, IconName, Sizable as _,
    button::{Button, ButtonVariants as _},
    dock::{BasePanel, Panel, PanelEvent},
    menu::PopupMenu,
    table::{Column, TableState},
    v_flex,
};
use gpui_kit::{prelude::*, *};
use mongodb::Collection;
use mongodb::bson::{Document, doc, to_document};

use gpui_kit::component::table::TableEvent;

use crate::connection::ConnectionId;
use crate::db;
use crate::widgets::cell_detail::CellDetail;
use crate::widgets::data_table::{configure_row_table, render_row_table};
use crate::widgets::empty_state::status_panel;
use crate::widgets::export;
use crate::widgets::filter_bar::{FilterBar, FilterEvent, FilterSet};
use crate::widgets::load::{Load, LoadFailure};
use crate::widgets::panel::data_viewer_toolbar;
use crate::widgets::panel::{
    panel_tab_content, tab_breadcrumb_footer, tab_breadcrumb_for_connection,
};
use crate::widgets::virtual_table::{
    RowDelegate, align_meta_to_columns, cells, data_column, replace_table_data,
};
use crate::workspace::notify::push_export_success;
use crate::workspace::tabs::kind::names;
use gpui_kit::component::shimmer::ShimmerText;

/// The filter as a query document. An error rather than `{}`: an empty filter
/// would quietly match every document.
fn mongo_filter_doc(expr: &FilterSet) -> Result<Document, String> {
    let s = expr.to_mongo_filter();
    let json = serde_json::from_str::<serde_json::Value>(&s).map_err(|e| e.to_string())?;
    to_document(&json).map_err(|e| e.to_string())
}

pub struct DocumentViewerPanel {
    focus_handle: FocusHandle,
    conn_id: ConnectionId,
    collection: Collection<Document>,
    table: Entity<TableState<RowDelegate>>,
    cell_detail: Entity<CellDetail>,
    filter_bar: Entity<FilterBar>,
    limit: i64,
    load: Load,
    pub(crate) tab_label: SharedString,
}

impl DocumentViewerPanel {
    pub fn new(
        collection: Collection<Document>,
        conn_id: ConnectionId,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Self {
        let delegate = RowDelegate::default();
        let tab_label = collection.name().to_string().into();
        let table = cx.new(|cx| configure_row_table(delegate, window, cx));
        let filter_bar = cx.new(|cx| FilterBar::new(window, cx, vec![]));
        cx.subscribe(&filter_bar, |panel, _, _: &FilterEvent, cx| {
            panel.reload(cx)
        })
        .detach();
        let cell_detail = cx.new(|_| CellDetail::default());
        let mut p = Self {
            focus_handle: cx.focus_handle(),
            conn_id,
            collection,
            table,
            cell_detail,
            filter_bar,
            limit: 200,
            load: Load::Idle,
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

    fn cell_snapshot(&self, row: usize, col: usize, cx: &App) -> Option<(String, Value)> {
        let st = self.table.read(cx);
        let del = st.delegate();
        let col_name = del.columns.get(col)?.key.to_string();
        let cell = del.rows.get(row)?.get(col)?;
        Some((col_name, cell.value.clone()))
    }

    fn reload(&mut self, cx: &mut Context<Self>) {
        let coll = self.collection.clone();
        let lim = self.limit;
        let filter_doc = match self.filter_bar.read(cx).current_set(cx) {
            None => doc! {},
            Some(set) => match mongo_filter_doc(&set) {
                Ok(doc) => doc,
                Err(e) => {
                    self.load = Load::failed("The filter isn't a valid query", e);
                    cx.notify();
                    return;
                }
            },
        };
        self.load = Load::Loading;

        cx.spawn(async move |this, cx| {
            let docs = match db::run(cx, async move {
                let mut cursor = coll.find(filter_doc).limit(lim).await?;
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
                Err(e) => {
                    let _ = cx.update(|cx| {
                        this.update(cx, |panel, cx| {
                            panel.load = Load::failed("Couldn't load documents", e);
                            cx.notify();
                        })
                    });
                    return;
                }
            };

            let mut set = documents_to_result_set(&docs);
            // The `_id` column is always there, even for no documents or a projection.
            if !set.column_names().any(|k| k == "_id") {
                set.columns.insert(0, ResultColumn::new("_id", None));
                for row in &mut set.rows {
                    row.insert(0, Value::Missing);
                }
            }
            let keys: Vec<String> = set.column_names().map(str::to_string).collect();
            let columns: Vec<Column> = keys
                .iter()
                .map(|k| data_column(k.clone(), k.clone()))
                .collect();
            let rows = cells(set.rows);

            let _ = cx.update(|cx| {
                this.update(cx, |panel, cx| {
                    panel.load = Load::Loaded;
                    panel.filter_bar.update(cx, |fb, cx| {
                        fb.set_columns_if_empty(keys.clone(), cx);
                    });
                    let column_meta = align_meta_to_columns(
                        keys.iter().map(String::as_str),
                        &ColumnCatalog::default(),
                    );
                    panel.table.update(cx, |state, cx| {
                        replace_table_data(state, columns, rows, column_meta, cx);
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

impl BasePanel for DocumentViewerPanel {
    crate::based_panel_behavior!(names::MONGO_DOCUMENT_VIEWER);
}

impl Panel for DocumentViewerPanel {
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
        let muted = cx.theme().muted_foreground;

        let (export_headers, export_rows) = {
            let st = self.table.read(cx);
            let d = st.delegate();
            let h = d
                .columns
                .iter()
                .map(|c| c.key.to_string())
                .collect::<Vec<_>>();
            let r = d.rows.clone();
            (h, r)
        };

        let body = v_flex()
            .relative()
            .size_full()
            .child(
                data_viewer_toolbar(cx)
                    .child(
                        Button::new("mongo-refresh-docs")
                            .small()
                            .icon(IconName::RefreshCw)
                            .label("Refresh")
                            .on_click(cx.listener(|p, _, _, cx| p.reload(cx))),
                    )
                    .child(self.filter_bar.clone())
                    .when(self.load.is_loading(), |h| {
                        h.child(
                            div()
                                .text_sm()
                                .text_color(muted)
                                .child(ShimmerText::new("Loading…")),
                        )
                    })
                    .child(
                        Button::new("mongo-export-json")
                            .outline()
                            .small()
                            .icon(Icon::new(AssetIcon::Download))
                            .label("Export JSON")
                            .on_click(move |_, _, cx| {
                                let json = export::to_json(&export_headers, &export_rows);
                                cx.spawn(async move |cx| {
                                    if let Some(path) = export::save_bytes(
                                        cx,
                                        "export.json",
                                        "JSON",
                                        &["json"],
                                        json.into_bytes(),
                                    )
                                    .await
                                    {
                                        cx.update(|app| push_export_success(app, &path));
                                    }
                                })
                                .detach();
                            }),
                    ),
            )
            .child(
                div()
                    .relative()
                    .flex_1()
                    .min_h_0()
                    .child(render_row_table(&self.table, cx))
                    .when_some(self.error_overlay(cx), |d, o| {
                        d.child(
                            div()
                                .absolute()
                                .inset_0()
                                .bg(cx.theme().background)
                                .child(o),
                        )
                    }),
            )
            .child(self.cell_detail.clone());
        let crumbs =
            tab_breadcrumb_for_connection(&self.conn_id, [self.collection.name().to_string()], cx);
        panel_tab_content(
            body,
            tab_breadcrumb_footer("mongo-docs-breadcrumb", crumbs, None, cx),
        )
    }
}

impl DocumentViewerPanel {
    /// The last load's failure, with Retry and Copy details, over the grid.
    fn error_overlay(&self, cx: &mut Context<Self>) -> Option<AnyElement> {
        let LoadFailure { title, detail } = self.load.failure()?.clone();
        let copy = detail.to_string();
        Some(status_panel(
            title,
            "Check the filter and the connection, then try again.",
            IconName::TriangleAlert,
            Some(cx.theme().danger),
            Some(detail),
            vec![
                Button::new("mongo-retry")
                    .small()
                    .primary()
                    .label("Retry")
                    .on_click(cx.listener(|p, _, _, cx| p.reload(cx)))
                    .into_any_element(),
                Button::new("mongo-copy-error")
                    .small()
                    .outline()
                    .label("Copy details")
                    .on_click(move |_, _, cx| {
                        cx.write_to_clipboard(ClipboardItem::new_string(copy.clone()));
                    })
                    .into_any_element(),
            ],
            cx,
        ))
    }
}
