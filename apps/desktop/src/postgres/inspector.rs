// postgres::inspector — columns, indexes, constraints, and stats with tabs.

use gpui::{prelude::*, *};
use gpui_component::{
    ActiveTheme,
    button::Button,
    dock::{Panel, PanelEvent},
    h_flex,
    menu::PopupMenu,
    table::TableState,
    v_flex,
};
use sqlx::{PgPool, Row};

use crate::widgets::compact_description_list_vertical;
use crate::widgets::data_table::{configure_row_table, render_row_table};
use crate::widgets::panel::tab_button_styled;
use crate::widgets::virtual_table::{
    RowDelegate, data_column, empty_column_meta, replace_table_data,
};

#[derive(Clone, Copy, PartialEq, Eq, Default)]
enum PgInspectorTab {
    #[default]
    Columns,
    Indexes,
    Constraints,
    Stats,
}

pub struct TableInspectorPanel {
    focus_handle: FocusHandle,
    pool: PgPool,
    schema: String,
    table_name: String,
    columns_tbl: Entity<TableState<RowDelegate>>,
    indexes_tbl: Entity<TableState<RowDelegate>>,
    constraints_tbl: Entity<TableState<RowDelegate>>,
    stats_rows: Vec<(SharedString, SharedString)>,
    tab: PgInspectorTab,
    pub(crate) tab_label: SharedString,
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
        let columns_tbl = cx.new(|cx| configure_row_table(c1, window, cx));
        let c2 = RowDelegate::default();
        let indexes_tbl = cx.new(|cx| configure_row_table(c2, window, cx));
        let c3 = RowDelegate::default();
        let constraints_tbl = cx.new(|cx| configure_row_table(c3, window, cx));
        let tab_label = format!("{schema}.{table_name} (schema)").into();
        let mut p = Self {
            focus_handle: cx.focus_handle(),
            pool,
            schema,
            table_name,
            columns_tbl,
            indexes_tbl,
            constraints_tbl,
            stats_rows: vec![],
            tab: PgInspectorTab::default(),
            tab_label,
        };
        p.load(cx);
        p
    }

    fn load(&mut self, cx: &mut Context<Self>) {
        let pool = self.pool.clone();
        let schema = self.schema.clone();
        let table = self.table_name.clone();
        cx.spawn(async move |this, cx| {
            let loaded = crate::db::run_infallible(cx, async move {
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

                let con_rows = sqlx::query(
                    r"SELECT con.conname AS constraint_name,
                       CASE con.contype
                         WHEN 'p' THEN 'PRIMARY KEY'
                         WHEN 'u' THEN 'UNIQUE'
                         WHEN 'f' THEN 'FOREIGN KEY'
                         WHEN 'c' THEN 'CHECK'
                         ELSE con.contype::text
                       END AS constraint_type,
                       pg_get_constraintdef(con.oid) AS definition
                FROM pg_constraint con
                JOIN pg_class rel ON rel.oid = con.conrelid
                JOIN pg_namespace nsp ON nsp.oid = rel.relnamespace
                WHERE nsp.nspname = $1 AND rel.relname = $2
                ORDER BY con.conname",
                )
                .bind(&schema)
                .bind(&table)
                .fetch_all(&pool)
                .await
                .unwrap_or_default();

                let stat_rows = sqlx::query(
                    r"SELECT
                       c.reltuples::bigint AS estimate_rows,
                       pg_size_pretty(pg_relation_size(c.oid)) AS table_size,
                       pg_size_pretty(pg_indexes_size(c.oid)) AS indexes_size,
                       pg_size_pretty(pg_total_relation_size(c.oid)) AS total_size
                FROM pg_class c
                JOIN pg_namespace n ON n.oid = c.relnamespace
                WHERE n.nspname = $1 AND c.relname = $2 AND c.relkind IN ('r', 'p', 'm')",
                )
                .bind(&schema)
                .bind(&table)
                .fetch_optional(&pool)
                .await
                .unwrap_or(None);

                let col_columns = vec![
                    data_column("pos", "#"),
                    data_column("name", "Column"),
                    data_column("type", "Type"),
                    data_column("nullable", "NULL"),
                    data_column("default", "Default"),
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
                    data_column("name", "Index"),
                    data_column("def", "Definition"),
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

                let co_columns = vec![
                    data_column("name", "Constraint"),
                    data_column("typ", "Type"),
                    data_column("def", "Definition"),
                ];
                let co_data: Vec<Vec<SharedString>> = con_rows
                    .iter()
                    .map(|row| {
                        vec![
                            row.try_get::<String, _>("constraint_name")
                                .unwrap_or_default()
                                .into(),
                            row.try_get::<String, _>("constraint_type")
                                .unwrap_or_default()
                                .into(),
                            row.try_get::<String, _>("definition")
                                .unwrap_or_default()
                                .into(),
                        ]
                    })
                    .collect();

                let stats_rows: Vec<(SharedString, SharedString)> =
                    if let Some(sr) = stat_rows.as_ref() {
                        let rows_est: Option<i64> = sr.try_get("estimate_rows").ok();
                        let ts: String = sr
                            .try_get::<Option<String>, _>("table_size")
                            .ok()
                            .flatten()
                            .unwrap_or_default();
                        let isz: String = sr
                            .try_get::<Option<String>, _>("indexes_size")
                            .ok()
                            .flatten()
                            .unwrap_or_default();
                        let tot: String = sr
                            .try_get::<Option<String>, _>("total_size")
                            .ok()
                            .flatten()
                            .unwrap_or_default();
                        vec![
                            (
                                "Estimated rows".into(),
                                rows_est
                                    .map(|n| n.to_string())
                                    .unwrap_or_else(|| "—".into())
                                    .into(),
                            ),
                            ("Table size".into(), ts.into()),
                            ("Indexes size".into(), isz.into()),
                            ("Total size".into(), tot.into()),
                        ]
                    } else {
                        vec![(
                            "(no stats)".into(),
                            "relation not found or not a regular table".into(),
                        )]
                    };

                (
                    col_columns,
                    col_data,
                    ix_columns,
                    ix_data,
                    co_columns,
                    co_data,
                    stats_rows,
                )
            })
            .await;

            let Ok((col_columns, col_data, ix_columns, ix_data, co_columns, co_data, stats_rows)) =
                loaded
            else {
                return;
            };

            let _ = cx.update(|cx| {
                this.update(cx, |panel, cx| {
                    let col_meta = empty_column_meta(col_columns.len());
                    let ix_meta = empty_column_meta(ix_columns.len());
                    let co_meta = empty_column_meta(co_columns.len());
                    panel.columns_tbl.update(cx, |state, cx| {
                        replace_table_data(state, col_columns, col_data, col_meta, cx);
                    });
                    panel.indexes_tbl.update(cx, |state, cx| {
                        replace_table_data(state, ix_columns, ix_data, ix_meta, cx);
                    });
                    panel.constraints_tbl.update(cx, |state, cx| {
                        replace_table_data(state, co_columns, co_data, co_meta, cx);
                    });
                    panel.stats_rows = stats_rows;
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
        tab: PgInspectorTab,
        cx: &mut Context<Self>,
    ) -> Button {
        tab_button_styled(id, label, self.tab == tab).on_click(cx.listener(
            move |panel, _, _, cx| {
                panel.tab = tab;
                cx.notify();
            },
        ))
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

    crate::based_panel_tab_chrome!();
}

impl Render for TableInspectorPanel {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let border = cx.theme().border;
        let stats_rows = self.stats_rows.clone();

        let body: AnyElement = match self.tab {
            PgInspectorTab::Stats => div()
                .p_3()
                .child(compact_description_list_vertical(stats_rows, true))
                .into_any_element(),
            PgInspectorTab::Columns => render_row_table(&self.columns_tbl, cx).into_any_element(),
            PgInspectorTab::Indexes => render_row_table(&self.indexes_tbl, cx).into_any_element(),
            PgInspectorTab::Constraints => {
                render_row_table(&self.constraints_tbl, cx).into_any_element()
            }
        };

        v_flex()
            .size_full()
            .gap_2()
            .child(
                h_flex()
                    .gap_2()
                    .py_2()
                    .px_2()
                    .border_b_1()
                    .border_color(border)
                    .child(self.tab_button(
                        "pg-insp-columns",
                        "Columns",
                        PgInspectorTab::Columns,
                        cx,
                    ))
                    .child(self.tab_button(
                        "pg-insp-indexes",
                        "Indexes",
                        PgInspectorTab::Indexes,
                        cx,
                    ))
                    .child(self.tab_button(
                        "pg-insp-constraints",
                        "Constraints",
                        PgInspectorTab::Constraints,
                        cx,
                    ))
                    .child(self.tab_button("pg-insp-stats", "Stats", PgInspectorTab::Stats, cx)),
            )
            .child(
                div()
                    .flex_1()
                    .min_h(px(200.0))
                    .border_1()
                    .border_color(border)
                    .child(body),
            )
    }
}
