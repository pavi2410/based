// sqlite::eqp_viewer — EqpViewerPanel: EXPLAIN QUERY PLAN tree viewer.

use gpui::{prelude::*, *};
use gpui_component::{
    ActiveTheme,
    dock::{Panel, PanelEvent},
    menu::PopupMenu,
    v_flex,
};
use sqlx::{Row, SqlitePool};

pub struct EqpNode {
    pub id: i64,
    pub parent: i64,
    pub detail: String,
}

pub struct EqpViewerPanel {
    focus_handle: FocusHandle,
    pool: SqlitePool,
    sql: String,
    nodes: Vec<EqpNode>,
}

impl EqpViewerPanel {
    pub fn new(pool: SqlitePool, sql: String, _window: &mut Window, cx: &mut Context<Self>) -> Self {
        let mut panel = Self {
            focus_handle: cx.focus_handle(),
            pool,
            sql,
            nodes: vec![],
        };
        panel.load_plan(cx);
        panel
    }

    fn load_plan(&mut self, cx: &mut Context<Self>) {
        let pool = self.pool.clone();
        let sql = format!("EXPLAIN QUERY PLAN {}", self.sql);

        cx.spawn(async move |this, cx| {
            let rows = match crate::db::run(cx, async move {
                Ok(sqlx::query(&sql).fetch_all(&pool).await?)
            }).await {
                Ok(r) => r,
                Err(_) => return,
            };

            let nodes: Vec<EqpNode> = rows
                .iter()
                .map(|row| {
                    let id: i64 = row.try_get("id").unwrap_or(0);
                    let parent: i64 = row.try_get("parent").unwrap_or(0);
                    let detail: String = row.try_get("detail").unwrap_or_default();
                    EqpNode { id, parent, detail }
                })
                .collect();

            let _ = cx.update(|cx| {
                this.update(cx, |panel, cx| {
                    panel.nodes = nodes;
                    cx.notify();
                })
            });
        })
        .detach();
    }

    fn depth_of(id: i64, nodes: &[EqpNode]) -> usize {
        let mut current = id;
        let mut depth = 0usize;
        loop {
            let parent = nodes.iter().find(|n| n.id == current).map(|n| n.parent);
            match parent {
                Some(p) if p != 0 && p != current => {
                    current = p;
                    depth += 1;
                    if depth > 64 {
                        break;
                    }
                }
                _ => break,
            }
        }
        depth
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

impl Render for EqpViewerPanel {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let fg = cx.theme().foreground;
        let rows: Vec<_> = self
            .nodes
            .iter()
            .map(|node| {
                let depth = Self::depth_of(node.id, &self.nodes);
                let detail: SharedString = node.detail.clone().into();
                div()
                    .w_full()
                    .py(px(2.0))
                    .pl(px((depth * 16) as f32 + 8.0))
                    .pr(px(8.0))
                    .text_sm()
                    .text_color(fg)
                    .child(detail)
            })
            .collect();

        v_flex()
            .id("eqp-scroll")
            .w_full()
            .h_full()
            .overflow_y_scroll()
            .p(px(8.0))
            .children(rows)
    }
}
