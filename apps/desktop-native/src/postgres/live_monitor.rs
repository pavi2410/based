// postgres::live_monitor — pg_stat_activity snapshot.

use gpui::{prelude::*, *};
use gpui_component::{
    ActiveTheme,
    button::Button,
    dock::{Panel, PanelEvent},
    h_flex, v_flex,
    table::{Column as TableColumn, DataTable, TableState},
};
use sqlx::{Column, PgPool, Row};

use crate::widgets::virtual_table::RowDelegate;

pub struct LiveMonitorPanel {
    focus_handle: FocusHandle,
    pool: PgPool,
    table: Entity<TableState<RowDelegate>>,
}

impl LiveMonitorPanel {
    pub fn new(pool: PgPool, window: &mut Window, cx: &mut Context<Self>) -> Self {
        let delegate = RowDelegate::default();
        let table = cx.new(|cx| TableState::new(delegate, window, cx).row_selectable(true));
        let mut p = Self {
            focus_handle: cx.focus_handle(),
            pool,
            table,
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
                        .map(|c| TableColumn::new(c.name().to_string(), c.name().to_string()))
                        .collect()
                } else {
                    vec![]
                };
                let data: Vec<Vec<SharedString>> = rows
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
                (columns, data)
            }).await {
                Ok(x) => x,
                Err(_) => return,
            };
            let _ = this.update(cx, |panel, cx| {
                panel.table.update(cx, |state, cx| {
                    let d = state.delegate_mut();
                    d.columns = columns;
                    d.rows = data;
                    cx.notify();
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

    fn closable(&self, _: &App) -> bool {
        true
    }

    fn title(&mut self, _: &mut Window, _cx: &mut Context<Self>) -> impl IntoElement {
        "pg_stat_activity"
    }
}

impl Render for LiveMonitorPanel {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let border = cx.theme().border;
        v_flex()
            .size_full()
            .child(
                h_flex()
                    .p_2()
                    .border_b_1()
                    .border_color(border)
                    .child(
                        Button::new("pg-refresh-activity")
                            .label("Refresh")
                            .on_click(cx.listener(|panel, _, _, cx| panel.refresh(cx))),
                    ),
            )
            .child(DataTable::new(&self.table).stripe(true).bordered(false))
    }
}
