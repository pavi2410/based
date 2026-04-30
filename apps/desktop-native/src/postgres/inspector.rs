// postgres::inspector — columns and indexes from information_schema / pg_indexes.

use gpui::{prelude::*, *};
use gpui_component::{
    ActiveTheme,
    dock::{Panel, PanelEvent},
    menu::PopupMenu,
    v_flex,
    table::{Column, DataTable, TableState},
};
use sqlx::{PgPool, Row};

use crate::widgets::virtual_table::RowDelegate;

pub struct TableInspectorPanel {
    focus_handle: FocusHandle,
    pool: PgPool,
    schema: String,
    table_name: String,
    columns_tbl: Entity<TableState<RowDelegate>>,
    indexes_tbl: Entity<TableState<RowDelegate>>,
}

impl TableInspectorPanel {
    pub fn new(
        pool: PgPool,
        schema: String,
        table_name: String,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Self {
        let c1 = RowDelegate::default();
        let columns_tbl = cx.new(|cx| TableState::new(c1, window, cx));
        let c2 = RowDelegate::default();
        let indexes_tbl = cx.new(|cx| TableState::new(c2, window, cx));
        let mut p = Self {
            focus_handle: cx.focus_handle(),
            pool,
            schema,
            table_name,
            columns_tbl,
            indexes_tbl,
        };
        p.load(cx);
        p
    }

    fn load(&mut self, cx: &mut Context<Self>) {
        let pool = self.pool.clone();
        let schema = self.schema.clone();
        let table = self.table_name.clone();
        cx.spawn(async move |this, cx| {
            let (col_columns, col_data, ix_columns, ix_data) = match crate::db::run_infallible(cx, async move {
                let col_rows = sqlx::query(
                    r"SELECT ordinal_position, column_name, data_type, is_nullable, column_default
                   FROM information_schema.columns
                   WHERE table_schema = $1 AND table_name = $2
                   ORDER BY ordinal_position",
                )
                .bind(&schema)
                .bind(&table)
                .fetch_all(&pool)
                .await
                .unwrap_or_default();

                let idx_rows = sqlx::query(
                    r"SELECT indexname, indexdef FROM pg_indexes
                   WHERE schemaname = $1 AND tablename = $2
                   ORDER BY indexname",
                )
                .bind(&schema)
                .bind(&table)
                .fetch_all(&pool)
                .await
                .unwrap_or_default();

                let col_columns = vec![
                    Column::new("pos", "#"),
                    Column::new("name", "Column"),
                    Column::new("type", "Type"),
                    Column::new("nullable", "NULL"),
                    Column::new("default", "Default"),
                ];
                let col_data: Vec<Vec<SharedString>> = col_rows
                    .iter()
                    .map(|row| {
                        let pos: i32 = row.try_get("ordinal_position").unwrap_or(0);
                        let name: String = row.try_get("column_name").unwrap_or_default();
                        let ty: String = row.try_get("data_type").unwrap_or_default();
                        let null: String = row.try_get("is_nullable").unwrap_or_default();
                        let def: String = row
                            .try_get::<Option<String>, _>("column_default")
                            .ok()
                            .flatten()
                            .unwrap_or_default();
                        vec![
                            pos.to_string().into(),
                            name.into(),
                            ty.into(),
                            null.into(),
                            def.into(),
                        ]
                    })
                    .collect();

                let ix_columns = vec![
                    Column::new("name", "Index"),
                    Column::new("def", "Definition"),
                ];
                let ix_data: Vec<Vec<SharedString>> = idx_rows
                    .iter()
                    .map(|row| {
                        vec![
                            row.try_get::<String, _>("indexname")
                                .unwrap_or_default()
                                .into(),
                            row.try_get::<String, _>("indexdef")
                                .unwrap_or_default()
                                .into(),
                        ]
                    })
                    .collect();

                (col_columns, col_data, ix_columns, ix_data)
            }).await {
                Ok(p) => p,
                Err(_) => return,
            };

            let _ = cx.update(|cx| {
                this.update(cx, |panel, cx| {
                    panel.columns_tbl.update(cx, |state, cx| {
                        let d = state.delegate_mut();
                        d.columns = col_columns;
                        d.rows = col_data;
                        cx.notify();
                    });
                    panel.indexes_tbl.update(cx, |state, cx| {
                        let d = state.delegate_mut();
                        d.columns = ix_columns;
                        d.rows = ix_data;
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
        "PgTableInspector"
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
        format!("{}.{} — schema", self.schema, self.table_name)
    }
}

impl Render for TableInspectorPanel {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let border = cx.theme().border;
        v_flex()
            .size_full()
            .gap_2()
            .p_2()
            .child(
                div()
                    .text_sm()
                    .font_weight(FontWeight::SEMIBOLD)
                    .child("Columns"),
            )
            .child(
                div()
                    .flex_1()
                    .min_h(px(160.0))
                    .border_1()
                    .border_color(border)
                    .child(DataTable::new(&self.columns_tbl).bordered(false)),
            )
            .child(
                div()
                    .text_sm()
                    .font_weight(FontWeight::SEMIBOLD)
                    .child("Indexes"),
            )
            .child(
                div()
                    .flex_1()
                    .min_h(px(120.0))
                    .border_1()
                    .border_color(border)
                    .child(DataTable::new(&self.indexes_tbl).bordered(false)),
            )
    }
}
