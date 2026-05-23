// sqlite::pragma_browser — PragmaBrowserPanel: displays key PRAGMA values.

use gpui::{prelude::*, *};
use gpui_component::{
    dock::{Panel, PanelEvent},
    menu::PopupMenu,
    table::{Column, TableState},
    v_flex,
};
use sqlx::{Row, SqlitePool};

use crate::widgets::data_table::read_only_striped;
use crate::widgets::virtual_table::{RowDelegate, replace_table_rows};
use crate::workspace::pop_out::PopOutWindowTitle;

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
    pub(crate) tab_label: SharedString,
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
            tab_label: "PRAGMAs".into(),
        };
        panel.load_pragmas(cx);
        panel
    }

    fn load_pragmas(&mut self, cx: &mut Context<Self>) {
        let pool = self.pool.clone();
        cx.spawn(async move |this, cx| {
            let rows = match crate::db::run_infallible(cx, async move {
                let mut rows: Vec<(String, String)> = vec![];
                for &name in PRAGMA_LIST {
                    let sql = format!("PRAGMA {name}");
                    let value = match sqlx::query(&sql).fetch_optional(&pool).await {
                        Ok(Some(row)) => {
                            let parts: Vec<String> = (0..row.len())
                                .map(|i| crate::widgets::row_cell::sqlite_cell_display(&row, i))
                                .collect();
                            parts.join(", ")
                        }
                        Ok(None) => String::new(),
                        Err(_) => String::new(),
                    };
                    rows.push((name.to_string(), value));
                }
                rows
            })
            .await
            {
                Ok(r) => r,
                Err(_) => return,
            };

            let data_rows: Vec<Vec<SharedString>> = rows
                .iter()
                .map(|(k, v)| vec![SharedString::from(k.clone()), SharedString::from(v.clone())])
                .collect();

            let _ = this.update(cx, |panel, cx| {
                panel.table.update(cx, |state, cx| {
                    replace_table_rows(state, data_rows, cx);
                });
                cx.notify();
            });
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

impl PopOutWindowTitle for PragmaBrowserPanel {
    fn pop_out_window_title(&mut self, _: &mut Window, _: &mut App) -> String {
        "PRAGMAs".into()
    }
}

impl Render for PragmaBrowserPanel {
    fn render(&mut self, _window: &mut Window, _cx: &mut Context<Self>) -> impl IntoElement {
        v_flex()
            .w_full()
            .h_full()
            .child(read_only_striped(&self.table))
    }
}
