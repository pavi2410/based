// postgres::data_viewer — paginated table reader (schema-qualified).

use gpui::{prelude::*, *};
use gpui_component::{
    ActiveTheme, Sizable as _,
    button::{Button, ButtonVariants},
    dock::{Panel, PanelEvent},
    h_flex,
    menu::PopupMenu,
    popover::Popover,
    table::{Column, TableState},
    v_flex,
};
use sqlx::{AssertSqlSafe, Column as SqlxColumn, PgPool, Row};

use gpui_component::table::TableEvent;

use crate::app::prefs;
use crate::widgets::cell_detail::{CellDetail, CellValue, interpret_cell_display};
use crate::widgets::data_table::{configure_row_table, render_row_table};
use crate::widgets::export;
use crate::widgets::filter_bar::FilterBar;
use crate::widgets::pagination::{offset_for_page, sql_pagination_controls, sql_row_range_label};
use crate::widgets::row_cell::pg_cell_display;
use crate::widgets::ui::{metadata_pill, panel_shell};
use crate::widgets::virtual_table::{RowDelegate, data_column, replace_table_data};
use crate::workspace::pop_out::{PopOutManager, PopOutWindowTitle};

pub struct DataViewerPanel {
    focus_handle: FocusHandle,
    pool: PgPool,
    schema: String,
    table_name: String,
    table: Entity<TableState<RowDelegate>>,
    cell_detail: Entity<CellDetail>,
    filter_bar: Entity<FilterBar>,
    offset: u64,
    page_size: u64,
    total_rows: u64,
    loading: bool,
    pub(crate) tab_label: SharedString,
}

impl DataViewerPanel {
    pub fn new(
        pool: PgPool,
        schema: String,
        table_name: String,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Self {
        let delegate = RowDelegate::default();
        let table = cx.new(|cx| configure_row_table(delegate, window, cx));
        let filter_bar = cx.new(|cx| FilterBar::new(window, cx, vec![]));
        let cell_detail = cx.new(|_| CellDetail::new());

        let tab_label = format!("{schema}.{table_name}").into();
        let mut panel = Self {
            focus_handle: cx.focus_handle(),
            pool,
            schema,
            table_name,
            table,
            cell_detail,
            filter_bar,
            offset: 0,
            page_size: prefs::page_size(cx),
            total_rows: 0,
            loading: false,
            tab_label,
        };
        cx.subscribe(&panel.table, |panel, _, event, cx| {
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
        panel.load_page(0, cx);
        panel
    }

    fn cell_snapshot(&self, row: usize, col: usize, cx: &App) -> Option<(String, CellValue)> {
        let st = self.table.read(cx);
        let del = st.delegate();
        let col_name = del.columns.get(col)?.key.to_string();
        let txt = del.rows.get(row)?.get(col)?.to_string();
        Some((col_name, interpret_cell_display(&txt)))
    }

    fn sql_identifier(ident: &str) -> String {
        ident.replace('"', "\"\"")
    }

    fn load_page(&mut self, offset: u64, cx: &mut Context<Self>) {
        self.loading = true;
        self.offset = offset;
        let pool = self.pool.clone();
        let schema = Self::sql_identifier(&self.schema);
        let table = Self::sql_identifier(&self.table_name);
        let page_size = self.page_size;
        let where_sql = self
            .filter_bar
            .read(cx)
            .current_expr(cx)
            .map(|e| e.to_sql_postgres());

        cx.spawn(async move |this, cx| {
            let res = crate::db::run(cx, async move {
                let where_clause = where_sql
                    .as_ref()
                    .map(|w| format!(" WHERE {w}"))
                    .unwrap_or_default();

                let count_sql =
                    format!(r#"SELECT COUNT(*) FROM "{schema}"."{table}"{where_clause}"#);
                let total: i64 = sqlx::query_scalar(AssertSqlSafe(count_sql))
                    .fetch_one(&pool)
                    .await
                    .unwrap_or(0);

                let fetch_sql = format!(
                    r#"SELECT * FROM "{schema}"."{table}"{where_clause} LIMIT {page_size} OFFSET {offset}"#
                );
                let rows = sqlx::query(AssertSqlSafe(fetch_sql)).fetch_all(&pool).await?;

                let columns: Vec<Column> = if let Some(first) = rows.first() {
                    first
                        .columns()
                        .iter()
                        .map(|c| data_column(c.name().to_string(), c.name().to_string()))
                        .collect()
                } else {
                    vec![]
                };

                let data_rows: Vec<Vec<SharedString>> = rows
                    .iter()
                    .map(|row| {
                        (0..row.len())
                            .map(|i| SharedString::from(pg_cell_display(row, i)))
                            .collect()
                    })
                    .collect();

                Ok((total, columns, data_rows))
            })
            .await;

            let _ = this.update(cx, |panel, cx| match res {
                Ok((total, columns, data_rows)) => {
                    panel.total_rows = total as u64;
                    panel.loading = false;
                    let names: Vec<String> = columns.iter().map(|c| c.key.to_string()).collect();
                    panel.filter_bar.update(cx, |fb, cx| {
                        fb.set_columns_if_empty(names, cx);
                    });
                    panel.table.update(cx, |state, cx| {
                        replace_table_data(state, columns, data_rows, cx);
                    });
                    cx.notify();
                }
                Err(_) => {
                    panel.loading = false;
                    cx.notify();
                }
            });
        })
        .detach();
    }

    fn go_to_page(&mut self, page_1_based: usize, cx: &mut Context<Self>) {
        self.load_page(offset_for_page(page_1_based, self.page_size), cx);
    }
}

impl EventEmitter<PanelEvent> for DataViewerPanel {}

impl Focusable for DataViewerPanel {
    fn focus_handle(&self, _: &App) -> FocusHandle {
        self.focus_handle.clone()
    }
}

impl Panel for DataViewerPanel {
    fn panel_name(&self) -> &'static str {
        "PgDataViewer"
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

impl PopOutWindowTitle for DataViewerPanel {
    fn pop_out_window_title(&mut self, _: &mut Window, _: &mut App) -> String {
        format!("{}.{}", self.schema, self.table_name)
    }
}

impl Render for DataViewerPanel {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let total = self.total_rows;
        let offset = self.offset;
        let page_size = self.page_size;
        let loading = self.loading;
        let _title: SharedString = format!("{}.{}", self.schema, self.table_name).into();
        let row_info: SharedString = sql_row_range_label(total, offset, page_size).into();
        let panel = cx.entity().downgrade();
        let muted = cx.theme().muted_foreground;
        let border = cx.theme().border;

        let (export_headers, export_rows) = {
            let st = self.table.read(cx);
            let d = st.delegate();
            let h = d
                .columns
                .iter()
                .map(|c| c.key.to_string())
                .collect::<Vec<_>>();
            let r = d
                .rows
                .iter()
                .map(|row| row.iter().map(|c| c.to_string()).collect())
                .collect::<Vec<Vec<String>>>();
            (h, r)
        };
        let export_popover = {
            let (h, r) = (export_headers.clone(), export_rows.clone());
            let (h2, r2) = (export_headers.clone(), export_rows.clone());
            Popover::new("pg-dv-export-popover")
                .trigger(
                    Button::new("pg-dv-export-trigger")
                        .ghost()
                        .small()
                        .label("Export"),
                )
                .content(move |_, _, _| {
                    let (hc, rc) = (h.clone(), r.clone());
                    let (hx, rx) = (h2.clone(), r2.clone());
                    v_flex()
                        .gap(px(2.0))
                        .p(px(4.0))
                        .child(
                            Button::new("pg-dv-export-csv")
                                .ghost()
                                .small()
                                .label("CSV")
                                .on_click(move |_, _, cx| {
                                    let (hc, rc) = (hc.clone(), rc.clone());
                                    cx.spawn(async move |cx| {
                                        if let Ok(bytes) = export::to_csv(&hc, &rc) {
                                            let _ = export::save_bytes(
                                                cx,
                                                "export.csv",
                                                "CSV",
                                                &["csv"],
                                                bytes,
                                            )
                                            .await;
                                        }
                                    })
                                    .detach();
                                }),
                        )
                        .child(
                            Button::new("pg-dv-export-xlsx")
                                .ghost()
                                .small()
                                .label("Excel (.xlsx)")
                                .on_click(move |_, _, cx| {
                                    let (hx, rx) = (hx.clone(), rx.clone());
                                    cx.spawn(async move |cx| {
                                        if let Ok(bytes) = export::to_xlsx(&hx, &rx) {
                                            let _ = export::save_bytes(
                                                cx,
                                                "export.xlsx",
                                                "Excel",
                                                &["xlsx"],
                                                bytes,
                                            )
                                            .await;
                                        }
                                    })
                                    .detach();
                                }),
                        )
                })
        };

        let toolbar = h_flex()
            .w_full()
            .px(px(8.0))
            .py(px(6.0))
            .gap(px(8.0))
            .flex_wrap()
            .border_b_1()
            .border_color(border.opacity(0.72))
            .bg(cx.theme().muted.opacity(0.18))
            .child(metadata_pill("rows", row_info, cx))
            .child(metadata_pill("schema", self.schema.clone(), cx))
            .child(metadata_pill("mode", "read-only", cx))
            .child(self.filter_bar.clone())
            .child(
                Button::new("pg-filter-apply")
                    .label("Apply filter")
                    .on_click(cx.listener(|panel, _, _, cx| panel.load_page(0, cx))),
            )
            .child(
                Button::new("pg-filter-clear")
                    .label("Clear filter")
                    .on_click(cx.listener(|panel, _, window, cx| {
                        panel.filter_bar.update(cx, |fb, cx| {
                            fb.clear(window, cx);
                        });
                        panel.load_page(0, cx);
                    })),
            )
            .child(export_popover)
            .child(div().flex_1())
            .when(loading, |d| {
                d.child(div().text_sm().text_color(muted).child("Loading…"))
            })
            .child(
                sql_pagination_controls("pg-pager", total, offset, page_size, loading).on_click(
                    move |page, _, cx| {
                        if let Some(ent) = panel.upgrade() {
                            ent.update(cx, |panel, cx| panel.go_to_page(*page, cx));
                        }
                    },
                ),
            );

        let body = v_flex()
            .size_full()
            .child(toolbar)
            .child(
                div()
                    .flex_1()
                    .min_h_0()
                    .child(render_row_table(&self.table, cx)),
            )
            .child(self.cell_detail.clone());

        if PopOutManager::is_pop_out_panel(cx.entity().entity_id(), cx) {
            body.into_any_element()
        } else {
            panel_shell(cx, "", "Browse Postgres relation data", body).into_any_element()
        }
    }
}
