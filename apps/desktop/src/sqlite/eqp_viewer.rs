// sqlite::eqp_viewer — EqpViewerPanel: EXPLAIN QUERY PLAN tree viewer.

use gpui::{prelude::*, *};
use gpui_component::{
    ActiveTheme,
    dock::{Panel, PanelEvent},
    menu::PopupMenu,
    scroll::ScrollableElement,
    v_flex,
};
use sqlx::{Row, SqlitePool};

use super::eqp_parse::{EqpNode, parse_eqp};

pub struct EqpViewerPanel {
    focus_handle: FocusHandle,
    pool: SqlitePool,
    sql: String,
    roots: Vec<EqpNode>,
}

impl EqpViewerPanel {
    pub fn new(
        pool: SqlitePool,
        sql: String,
        _window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Self {
        let mut panel = Self {
            focus_handle: cx.focus_handle(),
            pool,
            sql,
            roots: vec![],
        };
        panel.load_plan(cx);
        panel
    }

    fn load_plan(&mut self, cx: &mut Context<Self>) {
        let pool = self.pool.clone();
        let sql = format!("EXPLAIN QUERY PLAN {}", self.sql);

        cx.spawn(async move |this, cx| {
            let rows =
                match crate::db::run(
                    cx,
                    async move { Ok(sqlx::query(&sql).fetch_all(&pool).await?) },
                )
                .await
                {
                    Ok(r) => r,
                    Err(_) => return,
                };

            let flat: Vec<(i64, i64, String)> = rows
                .iter()
                .map(|row| {
                    let id: i64 = row.try_get("id").unwrap_or(0);
                    let parent: i64 = row.try_get("parent").unwrap_or(0);
                    let detail: String = row.try_get("detail").unwrap_or_default();
                    (id, parent, detail)
                })
                .collect();
            let roots = parse_eqp(&flat);

            let _ = cx.update(|cx| {
                this.update(cx, |panel, cx| {
                    panel.roots = roots;
                    cx.notify();
                })
            });
        })
        .detach();
    }
}

impl EventEmitter<PanelEvent> for EqpViewerPanel {}

impl Focusable for EqpViewerPanel {
    fn focus_handle(&self, _: &App) -> FocusHandle {
        self.focus_handle.clone()
    }
}

impl Panel for EqpViewerPanel {
    fn panel_name(&self) -> &'static str {
        "SqliteEqpViewer"
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
        "Query Plan"
    }
}

fn render_eqp_node(node: &EqpNode, depth: usize, theme: &gpui_component::Theme) -> AnyElement {
    let warn = theme.warning;
    let fg = if node.is_table_scan {
        warn
    } else {
        theme.foreground
    };
    let row = div()
        .w_full()
        .py(px(2.0))
        .pl(px((depth * 16) as f32 + 8.0))
        .pr(px(8.0))
        .when(node.is_table_scan, |d| d.border_l_2().border_color(warn))
        .text_sm()
        .text_color(fg)
        .child(node.detail.clone());

    let children: Vec<AnyElement> = node
        .children
        .iter()
        .map(|c| render_eqp_node(c, depth + 1, theme))
        .collect();

    v_flex()
        .w_full()
        .child(row)
        .children(children)
        .into_any_element()
}

impl Render for EqpViewerPanel {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let theme = cx.theme();
        let rows: Vec<AnyElement> = self
            .roots
            .iter()
            .map(|root| render_eqp_node(root, 0, theme))
            .collect();

        v_flex()
            .id("eqp-scroll")
            .w_full()
            .h_full()
            .overflow_y_scrollbar()
            .p(px(8.0))
            .children(rows)
    }
}
