// sqlite::pragma_browser — PragmaBrowserPanel: displays key PRAGMA values.

use gpui::{prelude::*, *};
use gpui_component::{
    dock::{Panel, PanelEvent},
    menu::PopupMenu,
    v_flex,
};
use sqlx::{AssertSqlSafe, Row, SqlitePool};

use crate::widgets::compact_description_list_vertical;
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

pub struct PragmaBrowserPanel {
    focus_handle: FocusHandle,
    pool: SqlitePool,
    pragmas: Vec<(SharedString, SharedString)>,
    pub(crate) tab_label: SharedString,
}

impl PragmaBrowserPanel {
    pub fn new(pool: SqlitePool, _window: &mut Window, cx: &mut Context<Self>) -> Self {
        let mut panel = Self {
            focus_handle: cx.focus_handle(),
            pool,
            pragmas: vec![],
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
                    let value = match sqlx::query(AssertSqlSafe(sql)).fetch_optional(&pool).await {
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

            let pragmas: Vec<(SharedString, SharedString)> = rows
                .into_iter()
                .map(|(k, v)| (k.into(), v.into()))
                .collect();

            let _ = this.update(cx, |panel, cx| {
                panel.pragmas = pragmas;
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

    crate::based_panel_tab_chrome!();
}

impl PopOutWindowTitle for PragmaBrowserPanel {
    fn pop_out_window_title(&mut self, _: &mut Window, _: &mut App) -> String {
        "PRAGMAs".into()
    }
}

impl Render for PragmaBrowserPanel {
    fn render(&mut self, _window: &mut Window, _cx: &mut Context<Self>) -> impl IntoElement {
        let pragmas = self.pragmas.clone();
        v_flex()
            .w_full()
            .h_full()
            .p_3()
            .child(compact_description_list_vertical(pragmas, true))
    }
}
