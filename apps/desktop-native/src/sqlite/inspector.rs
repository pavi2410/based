// sqlite::inspector — TableInspectorPanel: schema info for a single table.

use gpui::{prelude::*, *};
use gpui_component::{
    ActiveTheme,
    dock::{Panel, PanelEvent},
    menu::PopupMenu,
    table::{Column, DataTable, TableState},
    v_flex,
};
use sqlx::{Row, SqlitePool};

use crate::widgets::virtual_table::RowDelegate;

pub struct ColumnInfo {
    pub cid: i64,
    pub name: String,
    pub type_name: String,
    pub notnull: bool,
    pub pk: i64,
}

pub struct IndexInfo {
    pub name: String,
    pub unique: bool,
}

pub struct TableInspectorPanel {
    focus_handle: FocusHandle,
    pool: SqlitePool,
    table_name: String,
    columns: Vec<ColumnInfo>,
    indexes: Vec<IndexInfo>,
    col_table: Entity<TableState<RowDelegate>>,
    idx_table: Entity<TableState<RowDelegate>>,
}

impl TableInspectorPanel {
    pub fn new(
        pool: SqlitePool,
        table_name: String,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Self {
        let col_delegate = RowDelegate {
            columns: vec![
                Column::new("cid", "cid"),
                Column::new("name", "Name"),
                Column::new("type", "Type"),
                Column::new("notnull", "NOT NULL"),
                Column::new("pk", "PK"),
            ],
            ..Default::default()
        };
        let col_table = cx.new(|cx| TableState::new(col_delegate, window, cx));

        let idx_delegate = RowDelegate {
            columns: vec![
                Column::new("name", "Name"),
                Column::new("unique", "Unique"),
            ],
            ..Default::default()
        };
        let idx_table = cx.new(|cx| TableState::new(idx_delegate, window, cx));

        let mut panel = Self {
            focus_handle: cx.focus_handle(),
            pool,
            table_name,
            columns: vec![],
            indexes: vec![],
            col_table,
            idx_table,
        };
        panel.load_info(cx);
        panel
    }

    fn load_info(&mut self, cx: &mut Context<Self>) {
        let pool = self.pool.clone();
        let table_name = self.table_name.clone();

        cx.spawn(async move |this, cx| {
            let loaded = crate::db::run(cx, async move {
                let col_sql = format!("PRAGMA table_info(\"{table_name}\")");
                let col_rows = sqlx::query(&col_sql).fetch_all(&pool).await?;
                let columns: Vec<ColumnInfo> = col_rows
                    .iter()
                    .map(|row| ColumnInfo {
                        cid: row.try_get("cid").unwrap_or(0),
                        name: row.try_get::<String, _>("name").unwrap_or_default(),
                        type_name: row.try_get::<String, _>("type").unwrap_or_default(),
                        notnull: row.try_get::<bool, _>("notnull").unwrap_or(false),
                        pk: row.try_get("pk").unwrap_or(0),
                    })
                    .collect();

                let idx_sql = format!("PRAGMA index_list(\"{table_name}\")");
                let idx_rows = sqlx::query(&idx_sql).fetch_all(&pool).await?;
                let indexes: Vec<IndexInfo> = idx_rows
                    .iter()
                    .map(|row| IndexInfo {
                        name: row.try_get::<String, _>("name").unwrap_or_default(),
                        unique: row.try_get::<bool, _>("unique").unwrap_or(false),
                    })
                    .collect();

                Ok((columns, indexes))
            })
            .await;

            let (columns, indexes) = match loaded {
                Ok(x) => x,
                Err(_) => return,
            };

            let col_data: Vec<Vec<SharedString>> = columns
                .iter()
                .map(|c| {
                    vec![
                        SharedString::from(c.cid.to_string()),
                        SharedString::from(c.name.clone()),
                        SharedString::from(c.type_name.clone()),
                        SharedString::from(if c.notnull { "YES" } else { "NO" }),
                        SharedString::from(c.pk.to_string()),
                    ]
                })
                .collect();

            let idx_data: Vec<Vec<SharedString>> = indexes
                .iter()
                .map(|i| {
                    vec![
                        SharedString::from(i.name.clone()),
                        SharedString::from(if i.unique { "YES" } else { "NO" }),
                    ]
                })
                .collect();

            let _ = cx.update(|cx| {
                this.update(cx, |panel, cx| {
                    panel.columns = columns;
                    panel.indexes = indexes;
                    panel.col_table.update(cx, |state, cx| {
                        state.delegate_mut().rows = col_data;
                        cx.notify();
                    });
                    panel.idx_table.update(cx, |state, cx| {
                        state.delegate_mut().rows = idx_data;
                        cx.notify();
                    });
                    cx.notify();
                })
            });
        })
        .detach();
    }
}

impl EventEmitter<PanelEvent> for TableInspectorPanel {}

impl Focusable for TableInspectorPanel {
    fn focus_handle(&self, _: &App) -> FocusHandle {
        self.focus_handle.clone()
    }
}

impl Panel for TableInspectorPanel {
    fn panel_name(&self) -> &'static str {
        "SqliteTableInspector"
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
        format!("{} (schema)", self.table_name)
    }
}

impl Render for TableInspectorPanel {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let fg = cx.theme().foreground;
        let col_section = v_flex()
            .gap(px(4.0))
            .child(
                div()
                    .text_sm()
                    .font_weight(FontWeight::SEMIBOLD)
                    .text_color(fg)
                    .child("Columns"),
            )
            .child(DataTable::new(&self.col_table).stripe(true).bordered(false));

        let idx_section = v_flex()
            .gap(px(4.0))
            .child(
                div()
                    .text_sm()
                    .font_weight(FontWeight::SEMIBOLD)
                    .text_color(fg)
                    .child("Indexes"),
            )
            .child(DataTable::new(&self.idx_table).stripe(true).bordered(false));

        v_flex()
            .w_full()
            .h_full()
            .p(px(8.0))
            .gap(px(16.0))
            .child(col_section)
            .child(idx_section)
    }
}
