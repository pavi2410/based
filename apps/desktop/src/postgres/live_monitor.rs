// postgres::live_monitor — pg_stat_activity snapshot.

use gpui::{prelude::*, *};
use gpui_component::{
    ActiveTheme,
    button::Button,
    dock::{Panel, PanelEvent},
    h_flex,
    menu::PopupMenu,
    table::{Column as TableColumn, TableState},
    v_flex,
};
use sqlx::{Column, PgPool, Row};

use crate::widgets::data_table::{configure_row_table, render_row_table};
use crate::widgets::row_cell::pg_cell_display;
use crate::widgets::virtual_table::{RowDelegate, data_column, replace_table_data};

pub struct LiveMonitorPanel {
    focus_handle: FocusHandle,
    pool: PgPool,
    table: Entity<TableState<RowDelegate>>,
    pub(crate) tab_label: SharedString,
}

impl LiveMonitorPanel {
    pub fn new(pool: PgPool, window: &mut Window, cx: &mut Context<Self>) -> Self {
        let delegate = RowDelegate::default();
        let table = cx.new(|cx| configure_row_table(delegate, window, cx));
        let mut p = Self {
            focus_handle: cx.focus_handle(),
            pool,
            table,
            tab_label: "pg_stat_activity".into(),
        };
        p.refresh(cx);
        p
    }

    pub fn refresh(&mut self, cx: &mut Context<Self>) {
        let pool = self.pool.clone();
        cx.spawn(async move |this, cx| {
            let (columns, data) = match crate::db::run_infallible(cx, async move {
                let sql = r"
                SELECT pid, usename, application_name, client_addr::text, state,
                       query_start::text, left(query, 200) AS query
                FROM pg_stat_activity
                WHERE datname = current_database()
                ORDER BY query_start NULLS LAST
                LIMIT 500";
                let rows = sqlx::query(sql).fetch_all(&pool).await.unwrap_or_default();
                let columns: Vec<TableColumn> = if let Some(first) = rows.first() {
                    first
                        .columns()
                        .iter()
                        .map(|c| data_column(c.name().to_string(), c.name().to_string()))
                        .collect()
                } else {
                    vec![]
                };
                let data: Vec<Vec<SharedString>> = rows
                    .iter()
                    .map(|row| {
                        (0..row.len())
                            .map(|i| SharedString::from(pg_cell_display(row, i)))
                            .collect()
                    })
                    .collect();
                (columns, data)
            })
            .await
            {
                Ok(x) => x,
                Err(_) => return,
            };
            let _ = this.update(cx, |panel, cx| {
                panel.table.update(cx, |state, cx| {
                    replace_table_data(state, columns, data, cx);
                });
                cx.notify();
            });
        })
        .detach();
    }
}

impl EventEmitter<PanelEvent> for LiveMonitorPanel {}

impl Focusable for LiveMonitorPanel {
    fn focus_handle(&self, _: &App) -> FocusHandle {
        self.focus_handle.clone()
    }
}

impl Panel for LiveMonitorPanel {
    fn panel_name(&self) -> &'static str {
        "PgLiveMonitor"
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

impl Render for LiveMonitorPanel {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let border = cx.theme().border;
        v_flex()
            .size_full()
            .child(
                h_flex().p_2().border_b_1().border_color(border).child(
                    Button::new("pg-refresh-activity")
                        .label("Refresh")
                        .on_click(cx.listener(|panel, _, _, cx| panel.refresh(cx))),
                ),
            )
            .child(render_row_table(&self.table, cx))
    }
}
