// sqlite::data_viewer — DataViewerPanel: paginated table data viewer.

use gpui::{prelude::*, *};
use gpui_component::{
    ActiveTheme, Disableable,
    button::Button,
    dock::{Panel, PanelEvent},
    menu::PopupMenu,
    h_flex, v_flex,
    table::{Column, DataTable, TableState},
};
use sqlx::{Column as SqlxColumn, Row, SqlitePool};

use crate::widgets::virtual_table::RowDelegate;
use crate::widgets::ui::{metadata_pill, panel_header};

pub struct DataViewerPanel {
    focus_handle: FocusHandle,
    pool: SqlitePool,
    table_name: String,
    table: Entity<TableState<RowDelegate>>,
    offset: u64,
    page_size: u64,
    total_rows: u64,
    loading: bool,
}

impl DataViewerPanel {
    pub fn new(
        pool: SqlitePool,
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

        let mut panel = Self {
            focus_handle: cx.focus_handle(),
            pool,
            table_name,
            table,
            offset: 0,
            page_size: 500,
            total_rows: 0,
            loading: false,
        };
        panel.load_page(0, cx);
        panel
    }

    fn load_page(&mut self, offset: u64, cx: &mut Context<Self>) {
        self.loading = true;
        self.offset = offset;

        let pool = self.pool.clone();
        let table_name = self.table_name.clone();
        let page_size = self.page_size;

        cx.spawn(async move |this, cx| {
            let res = crate::db::run(cx, async move {
                let count_sql = format!("SELECT COUNT(*) FROM \"{table_name}\"");
                let total: i64 = sqlx::query_scalar(&count_sql)
                    .fetch_one(&pool)
                    .await
                    .unwrap_or(0);

                let fetch_sql =
                    format!("SELECT * FROM \"{table_name}\" LIMIT {page_size} OFFSET {offset}");
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
            let _ = cx.update(|cx| {
                this.update(cx, |panel, cx| match res {
                    Ok((total, columns, data_rows)) => {
                        panel.total_rows = total as u64;
                        panel.loading = false;
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
                })
            });
        })
        .detach();
    }

    fn prev_page(&mut self, cx: &mut Context<Self>) {
        if self.offset >= self.page_size {
            let new_offset = self.offset - self.page_size;
            self.load_page(new_offset, cx);
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

    fn title(&mut self, _: &mut Window, _cx: &mut Context<Self>) -> impl IntoElement {
        self.table_name.clone()
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

        let table_name: SharedString = self.table_name.clone().into();
        let end = (offset + page_size).min(total);
        let row_info: SharedString = format!("{} – {} of {}", offset + 1, end, total).into();

        let muted = cx.theme().muted_foreground;
        let border = cx.theme().border;

        let toolbar = h_flex()
            .w_full()
            .px(px(8.0))
            .py(px(6.0))
            .gap(px(8.0))
            .border_b_1()
            .border_color(border.opacity(0.72))
            .bg(cx.theme().muted.opacity(0.18))
            .child(metadata_pill("rows", row_info, cx))
            .child(metadata_pill("page", page_size.to_string(), cx))
            .child(metadata_pill("mode", "read-only", cx))
            .child(div().flex_1())
            .when(loading, |d| {
                d.child(div().text_sm().text_color(muted).child("Loading…"))
            })
            .child(
                Button::new("prev")
                    .label("◀")
                    .disabled(!can_prev)
                    .on_click(cx.listener(|panel, _, _window, cx| panel.prev_page(cx))),
            )
            .child(
                Button::new("next")
                    .label("▶")
                    .disabled(!can_next)
                    .on_click(cx.listener(|panel, _, _window, cx| panel.next_page(cx))),
            );

        v_flex()
            .w_full()
            .h_full()
            .bg(cx.theme().background)
            .child(panel_header(
                table_name,
                "Browse data, filter rows, inspect cells",
                cx,
            ))
            .child(toolbar)
            .child(DataTable::new(&self.table).stripe(true).bordered(false))
    }
}
