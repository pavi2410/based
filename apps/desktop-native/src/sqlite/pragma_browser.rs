// sqlite::pragma_browser — PragmaBrowserPanel: displays key PRAGMA values.

use gpui::{prelude::*, *};
use gpui_component::{
    dock::{Panel, PanelEvent},
    table::{Column, DataTable, TableState},
    v_flex,
};
use sqlx::SqlitePool;

use crate::widgets::virtual_table::RowDelegate;

const PRAGMA_LIST: &[&str] = &[
    "page_size",
    "page_count",
    "journal_mode",
    "synchronous",
    "cache_size",
    "auto_vacuum",
    "freelist_count",
    "integrity_check",
    "wal_checkpoint",
];

pub struct PragmaRow {
    pub name: &'static str,
    pub value: String,
}

pub struct PragmaBrowserPanel {
    focus_handle: FocusHandle,
    pool: SqlitePool,
    pragmas: Vec<PragmaRow>,
    table: Entity<TableState<RowDelegate>>,
}

impl PragmaBrowserPanel {
    pub fn new(pool: SqlitePool, window: &mut Window, cx: &mut Context<Self>) -> Self {
        let delegate = RowDelegate {
            columns: vec![
                Column::new("pragma", "PRAGMA"),
                Column::new("value", "Value"),
            ],
            ..Default::default()
        };
        let table = cx.new(|cx| TableState::new(delegate, window, cx));

        let mut panel = Self {
            focus_handle: cx.focus_handle(),
            pool,
            pragmas: vec![],
            table,
        };
        panel.load_pragmas(cx);
        panel
    }

    fn load_pragmas(&mut self, cx: &mut Context<Self>) {
        let pool = self.pool.clone();
        cx.spawn(async move |this, cx| {
            let mut rows: Vec<(String, String)> = vec![];
            for &name in PRAGMA_LIST {
                let sql = format!("PRAGMA {name}");
                let val: Option<String> = sqlx::query_scalar(&sql)
                    .fetch_optional(&pool)
                    .await
                    .ok()
                    .flatten();
                rows.push((name.to_string(), val.unwrap_or_default()));
            }

            let data_rows: Vec<Vec<SharedString>> = rows
                .iter()
                .map(|(k, v)| {
                    vec![SharedString::from(k.clone()), SharedString::from(v.clone())]
                })
                .collect();

            cx.update(|cx| {
                this.update(cx, |panel, cx| {
                    panel.table.update(cx, |state, cx| {
                        state.delegate_mut().rows = data_rows;
                        cx.notify();
                    });
                    cx.notify();
                })
            })
        })
        .detach();
    }
}

impl EventEmitter<PanelEvent> for PragmaBrowserPanel {}

impl Focusable for PragmaBrowserPanel {
    fn focus_handle(&self, _: &App) -> FocusHandle {
        self.focus_handle.clone()
    }
}

impl Panel for PragmaBrowserPanel {
    fn panel_name(&self) -> &'static str {
        "SqlitePragmaBrowser"
    }

    fn closable(&self, _: &App) -> bool {
        true
    }

    fn title(&mut self, _: &mut Window, _cx: &mut Context<Self>) -> impl IntoElement {
        "PRAGMAs"
    }
}

impl Render for PragmaBrowserPanel {
    fn render(&mut self, _window: &mut Window, _cx: &mut Context<Self>) -> impl IntoElement {
        v_flex()
            .w_full()
            .h_full()
            .child(DataTable::new(&self.table).stripe(true).bordered(false))
    }
}
