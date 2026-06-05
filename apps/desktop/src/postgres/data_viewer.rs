// postgres::data_viewer — paginated table reader (schema-qualified).

use std::collections::HashMap;

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
use sqlx::{AssertSqlSafe, Column as SqlxColumn, PgPool, Row};

use gpui_component::table::TableEvent;

use crate::app::prefs;
use crate::connection::ConnectionId;
use crate::widgets::cell_detail::{CellDetail, CellValue, interpret_cell_with_meta};
use crate::widgets::column_header::GridColumnMeta;
use crate::widgets::data_table::{configure_row_table, render_row_table};
use crate::widgets::export_popover::export_popover;
use crate::widgets::filter_bar::FilterBar;
use crate::widgets::pagination::{offset_for_page, sql_pagination_controls, sql_row_range_label};
use crate::widgets::panel::{
    panel_tab_content, tab_breadcrumb_data_viewer_trailing, tab_breadcrumb_footer,
    tab_breadcrumb_for_connection,
};
use crate::widgets::row_cell::pg_cell_display;
use crate::widgets::virtual_table::{
    RowDelegate, align_meta_to_columns, data_column, replace_table_data,
};
use crate::workspace::pop_out::PopOutWindowTitle;

pub struct DataViewerPanel {
    focus_handle: FocusHandle,
    pool: PgPool,
    conn_id: ConnectionId,
    schema: String,
    table_name: String,
    table: Entity<TableState<RowDelegate>>,
    cell_detail: Entity<CellDetail>,
    filter_bar: Entity<FilterBar>,
    offset: u64,
    page_size: u64,
    total_rows: u64,
    loading: bool,
    last_load_ms: Option<u64>,
    column_catalog: HashMap<String, GridColumnMeta>,
    pub(crate) tab_label: SharedString,
}

fn columns_from_rows_or_catalog(
    rows: &[sqlx::postgres::PgRow],
    catalog: &HashMap<String, GridColumnMeta>,
) -> Vec<Column> {
    if let Some(first) = rows.first() {
        return first
            .columns()
            .iter()
            .map(|c| data_column(c.name().to_string(), c.name().to_string()))
            .collect();
    }
    let mut names: Vec<_> = catalog.keys().cloned().collect();
    names.sort();
    names
        .into_iter()
        .map(|n| data_column(n.clone(), n))
        .collect()
}

impl DataViewerPanel {
    pub fn new(
        pool: PgPool,
        conn_id: ConnectionId,
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
            conn_id,
            schema,
            table_name,
            table,
            cell_detail,
            filter_bar,
            offset: 0,
            page_size: prefs::page_size(cx),
            total_rows: 0,
            loading: false,
            last_load_ms: None,
            column_catalog: HashMap::new(),
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
        let meta = del.column_meta.get(col).cloned().unwrap_or_default();
        Some((col_name, interpret_cell_with_meta(&txt, &meta)))
    }

    fn sql_identifier(ident: &str) -> String {
        ident.replace('"', "\"\"")
    }

    fn load_page(&mut self, offset: u64, cx: &mut Context<Self>) {
        self.loading = true;
        self.offset = offset;
        let pool = self.pool.clone();
        let schema_raw = self.schema.clone();
        let table_raw = self.table_name.clone();
        let schema = Self::sql_identifier(&schema_raw);
        let table = Self::sql_identifier(&table_raw);
        let page_size = self.page_size;
        let where_sql = self
            .filter_bar
            .read(cx)
            .current_expr(cx)
            .map(|e| e.to_sql_postgres());
        let cached_catalog = self.column_catalog.clone();

        cx.spawn(async move |this, cx| {
            let catalog = if cached_catalog.is_empty() {
                let pool_for_catalog = pool.clone();
                crate::db::run(cx, async move {
                    crate::db::column_catalog::load_postgres_column_catalog(
                        &pool_for_catalog,
                        &schema_raw,
                        &table_raw,
                    )
                    .await
                })
                .await
                .unwrap_or_default()
            } else {
                cached_catalog
            };

            let catalog_for_rows = catalog.clone();
            let res = crate::db::run(cx, async move {
                let start = std::time::Instant::now();
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

                let columns: Vec<Column> =
                    columns_from_rows_or_catalog(&rows, &catalog_for_rows);

                let data_rows: Vec<Vec<SharedString>> = rows
                    .iter()
                    .map(|row| {
                        (0..row.len())
                            .map(|i| SharedString::from(pg_cell_display(row, i)))
                            .collect()
                    })
                    .collect();

                let elapsed_ms = start.elapsed().as_millis() as u64;
                Ok((total, columns, data_rows, elapsed_ms))
            })
            .await;

            let _ = this.update(cx, |panel, cx| match res {
                Ok((total, columns, data_rows, elapsed_ms)) => {
                    panel.total_rows = total as u64;
                    panel.loading = false;
                    panel.last_load_ms = Some(elapsed_ms);
                    panel.column_catalog = catalog.clone();
                    let names: Vec<String> = columns.iter().map(|c| c.key.to_string()).collect();
                    panel.filter_bar.update(cx, |fb, cx| {
                        fb.set_columns_if_empty(names, cx);
                    });
                    let column_meta = align_meta_to_columns(
                        columns.iter().map(|c| c.key.to_string()),
                        &catalog,
                    );
                    panel.table.update(cx, |state, cx| {
                        replace_table_data(state, columns, data_rows, column_meta, cx);
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
        let export_popover = export_popover("pg-dv", export_headers, export_rows);

        let toolbar = h_flex()
            .w_full()
            .px(px(8.0))
            .py(px(6.0))
            .gap(px(8.0))
            .flex_wrap()
            .border_b_1()
            .border_color(border.opacity(0.72))
            .bg(cx.theme().muted.opacity(0.18))
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

        let crumbs = tab_breadcrumb_for_connection(
            &self.conn_id,
            [self.schema.clone(), self.table_name.clone()],
            cx,
        );
        let footer = tab_breadcrumb_footer(
            "pg-dv-breadcrumb",
            crumbs,
            Some(tab_breadcrumb_data_viewer_trailing(
                row_info,
                self.last_load_ms,
                "pg-dv-read-only",
                cx,
            )),
            cx,
        );

        panel_tab_content(body, footer).into_any_element()
    }
}
