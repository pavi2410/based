// postgres::explain — EXPLAIN (ANALYZE, FORMAT JSON) as a visual plan tree.

use gpui::{prelude::*, *};
use gpui_component::{
    ActiveTheme,
    button::Button,
    dock::{Panel, PanelEvent},
    h_flex,
    menu::PopupMenu,
    scroll::ScrollableElement,
    v_flex,
};
use sqlx::{PgPool, Row};

use super::explain_plan::{PlanNode, parse_pg_explain_json};

const SLOW_MS: f64 = 100.0;

pub struct ExplainPanel {
    focus_handle: FocusHandle,
    pool: PgPool,
    sql: String,
    plan_root: Option<PlanNode>,
    plan_text: String,
    use_analyze: bool,
}

impl ExplainPanel {
    pub fn new(pool: PgPool, sql: String, _window: &mut Window, cx: &mut Context<Self>) -> Self {
        let mut p = Self {
            focus_handle: cx.focus_handle(),
            pool,
            sql,
            plan_root: None,
            plan_text: String::new(),
            use_analyze: true,
        };
        p.refresh(cx);
        p
    }

    fn refresh(&mut self, cx: &mut Context<Self>) {
        let pool = self.pool.clone();
        let stmt = self.sql.clone();
        let analyze = self.use_analyze;
        cx.spawn(async move |this, cx| {
            let (root, text) = match crate::db::run_infallible(cx, async move {
                let prefix = if analyze {
                    "EXPLAIN (ANALYZE, FORMAT JSON)"
                } else {
                    "EXPLAIN (FORMAT JSON)"
                };
                let q = format!("{prefix} {stmt}");
                let rows = sqlx::query(&q).fetch_all(&pool).await.unwrap_or_default();
                let raw: String = rows
                    .first()
                    .and_then(|row| row.try_get::<String, usize>(0).ok())
                    .unwrap_or_default();
                if raw.is_empty() {
                    return (
                        None,
                        if rows.is_empty() {
                            "(no plan rows)".to_string()
                        } else {
                            "(could not read plan)".to_string()
                        },
                    );
                }
                match serde_json::from_str::<serde_json::Value>(&raw) {
                    Ok(json) => (parse_pg_explain_json(&json), raw),
                    Err(e) => (None, format!("(invalid JSON: {e})")),
                }
            })
            .await
            {
                Ok(pair) => pair,
                Err(_) => (None, "(error)".into()),
            };
            cx.update(|cx| {
                this.update(cx, |panel, cx| {
                    panel.plan_root = root;
                    panel.plan_text = text;
                    cx.notify();
                })
            })
        })
        .detach();
    }
}

impl EventEmitter<PanelEvent> for ExplainPanel {}

impl Focusable for ExplainPanel {
    fn focus_handle(&self, _: &App) -> FocusHandle {
        self.focus_handle.clone()
    }
}

impl Panel for ExplainPanel {
    fn panel_name(&self) -> &'static str {
        "PgExplain"
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
        "EXPLAIN plan"
    }
}

fn render_plan_node(node: &PlanNode, depth: usize, theme: &gpui_component::Theme) -> AnyElement {
    let slow = node.is_slow(SLOW_MS);
    let relation = node
        .relation
        .as_deref()
        .map(|r| format!(" on {r}"))
        .unwrap_or_default();
    let index = node
        .index_name
        .as_deref()
        .map(|i| format!(" ({i})"))
        .unwrap_or_default();
    let rows = match (node.rows_actual, node.rows_estimated) {
        (Some(actual), est) => format!("rows {actual} / est {est}"),
        (None, est) => format!("rows est {est}"),
    };
    let time = node
        .time_actual_ms
        .map(|t| format!(" — {t:.2} ms"))
        .unwrap_or_default();
    let title = format!("{}{}{} — {}{}", node.node_type, relation, index, rows, time);
    let warn = theme.warning;

    let row = div()
        .w_full()
        .py(px(4.0))
        .pl(px((depth * 16) as f32 + 8.0))
        .pr(px(8.0))
        .when(slow, |d| {
            d.border_l_2()
                .border_color(warn)
                .pl(px((depth * 16) as f32 + 6.0))
        })
        .child(
            div()
                .text_sm()
                .text_color(if slow { warn } else { theme.foreground })
                .child(title),
        );

    let children: Vec<AnyElement> = node
        .children
        .iter()
        .map(|c| render_plan_node(c, depth + 1, theme))
        .collect();

    v_flex()
        .w_full()
        .child(row)
        .children(children)
        .into_any_element()
}

impl Render for ExplainPanel {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let border = cx.theme().border;
        let analyze = self.use_analyze;
        let theme = cx.theme();
        let mono = theme.mono_font_family.clone();

        let body: AnyElement = if let Some(ref root) = self.plan_root {
            render_plan_node(root, 0, theme)
        } else {
            div()
                .font_family(mono)
                .text_sm()
                .text_color(theme.foreground)
                .child(self.plan_text.clone())
                .into_any_element()
        };

        v_flex()
            .id("pg-explain")
            .size_full()
            .child(
                h_flex()
                    .p_2()
                    .gap_2()
                    .border_b_1()
                    .border_color(border)
                    .child(
                        Button::new("pg-explain-refresh")
                            .label("Refresh")
                            .on_click(cx.listener(|panel, _, _, cx| panel.refresh(cx))),
                    )
                    .child(
                        Button::new("pg-explain-toggle")
                            .label(if analyze {
                                "ANALYZE: on"
                            } else {
                                "ANALYZE: off"
                            })
                            .on_click(cx.listener(|panel, _, _, cx| {
                                panel.use_analyze = !panel.use_analyze;
                                panel.refresh(cx);
                            })),
                    ),
            )
            .child(
                div()
                    .id("pg-explain-body")
                    .flex_1()
                    .overflow_y_scrollbar()
                    .p_3()
                    .child(body),
            )
    }
}
