// sqlite::data_viewer — DataViewerPanel: paginated table data viewer.

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
use sqlx::{Column as SqlxColumn, Row, SqlitePool};

use gpui_component::table::TableEvent;

use crate::app::prefs;
use crate::widgets::cell_detail::{CellDetail, CellValue, interpret_cell_display};
use crate::widgets::data_table::{configure_row_table, render_row_table};
use crate::widgets::filter_bar::FilterBar;
use crate::widgets::pagination::sql_page_state;
use crate::widgets::pagination::{offset_for_page, sql_pagination_controls, sql_row_range_label};
use crate::widgets::row_cell::sqlite_cell_display;
use crate::widgets::ui::{metadata_pill, panel_shell};
use crate::widgets::virtual_table::{RowDelegate, data_column, replace_table_data};
use crate::workspace::pop_out::{PopOutManager, PopOutWindowTitle};

pub struct DataViewerPanel {
    focus_handle: FocusHandle,
    pool: SqlitePool,
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
        pool: SqlitePool,
        table_name: String,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Self {
        let delegate = RowDelegate::default();
        let table = cx.new(|cx| configure_row_table(delegate, window, cx));
        let filter_bar = cx.new(|cx| FilterBar::new(window, cx, vec![]));
        let cell_detail = cx.new(|_| CellDetail::new());

        let tab_label = table_name.clone().into();
        let mut panel = Self {
            focus_handle: cx.focus_handle(),
            pool,
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

    fn sql_escape_ident(ident: &str) -> String {
        ident.replace('"', "\"\"")
    }

    fn load_page(&mut self, offset: u64, cx: &mut Context<Self>) {
        self.loading = true;
        self.offset = offset;

        let pool = self.pool.clone();
        let table_name = Self::sql_escape_ident(&self.table_name);
        let page_size = self.page_size;
        let where_sql = self
            .filter_bar
            .read(cx)
            .current_expr(cx)
            .map(|e| e.to_sql_sqlite());

        cx.spawn(async move |this, cx| {
            let res = crate::db::run(cx, async move {
                let where_clause = where_sql
                    .as_ref()
                    .map(|w| format!(" WHERE {w}"))
                    .unwrap_or_default();

                let count_sql = format!("SELECT COUNT(*) FROM \"{table_name}\"{where_clause}");
                let total: i64 = sqlx::query_scalar(&count_sql)
                    .fetch_one(&pool)
                    .await
                    .unwrap_or(0);

                let fetch_sql = format!(
                    "SELECT * FROM \"{table_name}\"{where_clause} LIMIT {page_size} OFFSET {offset}"
                );
                let rows = sqlx::query(&fetch_sql).fetch_all(&pool).await?;

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
                            .map(|i| SharedString::from(sqlite_cell_display(row, i)))
                            .collect()
                    })
                    .collect();

                Ok((total, columns, data_rows))
            })
            .await;
            let _ = cx.update(|cx| {
                this.update(cx, |panel, cx| match res {
                    Ok((total, columns, data_rows)) => {
                        panel.total_rows = total as u64;
                        panel.loading = false;
                        let names: Vec<String> =
                            columns.iter().map(|c| c.key.to_string()).collect();
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
                })
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
        "SqliteDataViewer"
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

    crate::based_panel_tab_chrome!();

    fn title(&mut self, _: &mut Window, _cx: &mut Context<Self>) -> impl IntoElement {
        self.tab_label.clone()
    }
}

impl PopOutWindowTitle for DataViewerPanel {
    fn pop_out_window_title(&mut self, _: &mut Window, _: &mut App) -> String {
        self.table_name.clone()
    }
}

impl Render for DataViewerPanel {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let total = self.total_rows;
        let offset = self.offset;
        let page_size = self.page_size;
        let loading = self.loading;

        let _table_name: SharedString = self.table_name.clone().into();
        let row_info: SharedString = sql_row_range_label(total, offset, page_size).into();
        let (current_page, total_pages) = sql_page_state(total, offset, page_size);
        let page_info: SharedString = format!("{current_page} / {total_pages}").into();
        let panel = cx.entity().downgrade();

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
            .child(metadata_pill("page", page_info, cx))
            .child(metadata_pill("mode", "read-only", cx))
            .child(self.filter_bar.clone())
            .child(
                Button::new("sqlite-filter-apply")
                    .label("Apply filter")
                    .on_click(cx.listener(|panel, _, _, cx| panel.load_page(0, cx))),
            )
            .child(
                Button::new("sqlite-filter-clear")
                    .label("Clear filter")
                    .on_click(cx.listener(|panel, _, window, cx| {
                        panel.filter_bar.update(cx, |fb, cx| {
                            fb.clear(window, cx);
                        });
                        panel.load_page(0, cx);
                    })),
            )
            .child(div().flex_1())
            .when(loading, |d| {
                d.child(div().text_sm().text_color(muted).child("Loading…"))
            })
            .child(
                sql_pagination_controls("sqlite-pager", total, offset, page_size, loading)
                    .on_click(move |page, _, cx| {
                        if let Some(ent) = panel.upgrade() {
                            ent.update(cx, |panel, cx| panel.go_to_page(*page, cx));
                        }
                    }),
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
            panel_shell(cx, "", "Browse data, filter rows, inspect cells", body).into_any_element()
        }
    }
}
