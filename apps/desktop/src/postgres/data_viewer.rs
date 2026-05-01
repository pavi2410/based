// postgres::data_viewer — paginated table reader (schema-qualified).

use gpui::{prelude::*, *};
use gpui_component::{
    ActiveTheme, Disableable,
    button::Button,
    dock::{Panel, PanelEvent},
    h_flex,
    menu::PopupMenu,
    table::{Column, DataTable, TableState},
    v_flex,
};
use sqlx::{Column as SqlxColumn, PgPool, Row};

use gpui_component::table::TableEvent;

use crate::widgets::cell_detail::{CellDetail, CellValue, interpret_cell_display};
use crate::widgets::filter_bar::FilterBar;
use crate::widgets::ui::{metadata_pill, panel_header};
use crate::widgets::virtual_table::RowDelegate;

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
        let table = cx.new(|cx| {
            TableState::new(delegate, window, cx)
                .row_selectable(true)
                .cell_selectable(true)
        });
        let filter_bar = cx.new(|cx| FilterBar::new(window, cx, vec![]));
        let cell_detail = cx.new(|_| CellDetail::new());

        let mut panel = Self {
            focus_handle: cx.focus_handle(),
            pool,
            schema,
            table_name,
            table,
            cell_detail,
            filter_bar,
            offset: 0,
            page_size: 500,
            total_rows: 0,
            loading: false,
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
                let total: i64 = sqlx::query_scalar(&count_sql)
                    .fetch_one(&pool)
                    .await
                    .unwrap_or(0);

                let fetch_sql = format!(
                    r#"SELECT * FROM "{schema}"."{table}"{where_clause} LIMIT {page_size} OFFSET {offset}"#
                );
                let rows = sqlx::query(&fetch_sql).fetch_all(&pool).await?;

                let columns: Vec<Column> = if let Some(first) = rows.first() {
                    first
                        .columns()
                        .iter()
                        .map(|c| Column::new(c.name().to_string(), c.name().to_string()))
                        .collect()
                } else {
                    vec![]
                };

                let data_rows: Vec<Vec<SharedString>> = rows
                    .iter()
                    .map(|row| {
                        (0..row.len())
                            .map(|i| {
                                let val: Option<String> = row.try_get(i).ok();
                                SharedString::from(val.unwrap_or_default())
                            })
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
                        let delegate = state.delegate_mut();
                        delegate.columns = columns;
                        delegate.rows = data_rows;
                        cx.notify();
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

    fn prev_page(&mut self, cx: &mut Context<Self>) {
        if self.offset >= self.page_size {
            self.load_page(self.offset - self.page_size, cx);
        }
    }

    fn next_page(&mut self, cx: &mut Context<Self>) {
        let new_offset = self.offset + self.page_size;
        if new_offset < self.total_rows {
            self.load_page(new_offset, cx);
        }
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

    fn closable(&self, _: &App) -> bool {
        true
    }

    fn title(&mut self, _: &mut Window, _cx: &mut Context<Self>) -> impl IntoElement {
        format!("{}.{}", self.schema, self.table_name)
    }
}

impl Render for DataViewerPanel {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let total = self.total_rows;
        let offset = self.offset;
        let page_size = self.page_size;
        let can_prev = offset > 0;
        let can_next = offset + page_size < total;
        let loading = self.loading;
        let title: SharedString = format!("{}.{}", self.schema, self.table_name).into();
        let end = (offset + page_size).min(total);
        let row_info: SharedString = format!("{} – {} of {}", offset + 1, end, total).into();
        let muted = cx.theme().muted_foreground;
        let border = cx.theme().border;

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
                Button::new("pg-filter-clear").label("Clear filter").on_click(cx.listener(
                    |panel, _, window, cx| {
                        panel.filter_bar.update(cx, |fb, cx| {
                            fb.clear(window, cx);
                        });
                        panel.load_page(0, cx);
                    },
                )),
            )
            .child(div().flex_1())
            .when(loading, |d| {
                d.child(div().text_sm().text_color(muted).child("Loading…"))
            })
            .child(
                Button::new("pg-prev")
                    .label("◀")
                    .disabled(!can_prev)
                    .on_click(cx.listener(|panel, _, _, cx| panel.prev_page(cx))),
            )
            .child(
                Button::new("pg-next")
                    .label("▶")
                    .disabled(!can_next)
                    .on_click(cx.listener(|panel, _, _, cx| panel.next_page(cx))),
            );

        v_flex()
            .relative()
            .w_full()
            .h_full()
            .bg(cx.theme().background)
            .child(panel_header(title, "Browse Postgres relation data", cx))
            .child(toolbar)
            .child(DataTable::new(&self.table).stripe(true).bordered(false))
            .child(self.cell_detail.clone())
    }
}
