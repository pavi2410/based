// sqlite::inspector — TableInspectorPanel: columns, indexes, DDL (PRAGMA + sqlite_master).

use gpui::{prelude::*, *};
use gpui_component::{
    ActiveTheme, Sizable,
    button::{Button, ButtonVariants},
    dock::{Panel, PanelEvent},
    h_flex,
    menu::PopupMenu,
    table::{Column, TableState},
    v_flex,
};
use sqlx::{Row, SqlitePool};

use crate::widgets::data_table::read_only_striped;
use crate::widgets::virtual_table::{RowDelegate, replace_table_rows};

#[derive(Clone, Copy, PartialEq, Eq, Default)]
enum SqliteInspectorTab {
    #[default]
    Columns,
    Indexes,
    Ddl,
}

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
    ddl_text: SharedString,
    col_table: Entity<TableState<RowDelegate>>,
    idx_table: Entity<TableState<RowDelegate>>,
    tab: SqliteInspectorTab,
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
            columns: vec![Column::new("name", "Name"), Column::new("unique", "Unique")],
            ..Default::default()
        };
        let idx_table = cx.new(|cx| TableState::new(idx_delegate, window, cx));

        let mut panel = Self {
            focus_handle: cx.focus_handle(),
            pool,
            table_name,
            columns: vec![],
            indexes: vec![],
            ddl_text: SharedString::from("(loading…)"),
            col_table,
            idx_table,
            tab: SqliteInspectorTab::default(),
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

                let ddl: String = sqlx::query_scalar::<_, String>(
                    r#"SELECT COALESCE(sql, '') FROM sqlite_master WHERE type IN ('table','view') AND name = ?"#,
                )
                .bind(&table_name)
                .fetch_optional(&pool)
                .await?
                .unwrap_or_default();

                Ok((columns, indexes, ddl))
            })
            .await;

            let (columns, indexes, ddl) = match loaded {
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

            let ddl_ss: SharedString = if ddl.is_empty() {
                SharedString::from("(no DDL in sqlite_master)")
            } else {
                ddl.into()
            };

            let _ = cx.update(|cx| {
                this.update(cx, |panel, cx| {
                    panel.columns = columns;
                    panel.indexes = indexes;
                    panel.ddl_text = ddl_ss;
                    panel.col_table.update(cx, |state, cx| {
                        replace_table_rows(state, col_data, cx);
                    });
                    panel.idx_table.update(cx, |state, cx| {
                        replace_table_rows(state, idx_data, cx);
                    });
                    cx.notify();
                })
            });
        })
        .detach();
    }

    fn tab_button(
        &self,
        id: &'static str,
        label: &'static str,
        tab: SqliteInspectorTab,
        cx: &mut Context<Self>,
    ) -> Button {
        let active = self.tab == tab;
        let mut b = Button::new(id).label(label).small();
        if active {
            b = b.outline();
        } else {
            b = b.ghost();
        }
        b.on_click(cx.listener(move |panel, _, _, cx| {
            panel.tab = tab;
            cx.notify();
        }))
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
        let border = cx.theme().border;
        let body: SharedString = self.ddl_text.clone();

        let tables_block = match self.tab {
            SqliteInspectorTab::Columns => div()
                .flex_1()
                .min_h(px(160.0))
                .child(read_only_striped(&self.col_table))
                .into_any_element(),
            SqliteInspectorTab::Indexes => div()
                .flex_1()
                .min_h(px(160.0))
                .child(read_only_striped(&self.idx_table))
                .into_any_element(),
            SqliteInspectorTab::Ddl => div()
                .id("sqlite-inspector-ddl")
                .flex_1()
                .min_h(px(160.0))
                .overflow_y_scroll()
                .p_3()
                .border_1()
                .border_color(border)
                .font_family("monospace")
                .text_sm()
                .text_color(fg)
                .child(body)
                .into_any_element(),
        };

        v_flex()
            .w_full()
            .h_full()
            .gap(px(8.0))
            .child(
                h_flex()
                    .gap_2()
                    .py_2()
                    .child(self.tab_button(
                        "sql-insp-col",
                        "Columns",
                        SqliteInspectorTab::Columns,
                        cx,
                    ))
                    .child(self.tab_button(
                        "sql-insp-ix",
                        "Indexes",
                        SqliteInspectorTab::Indexes,
                        cx,
                    ))
                    .child(self.tab_button("sql-insp-ddl", "DDL", SqliteInspectorTab::Ddl, cx)),
            )
            .child(tables_block)
    }
}
