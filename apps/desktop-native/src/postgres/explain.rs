// postgres::explain — EXPLAIN / EXPLAIN ANALYZE as plain-text plan.

use gpui::{prelude::*, *};
use gpui_component::{
    ActiveTheme,
    button::Button,
    dock::{Panel, PanelEvent},
    h_flex, v_flex,
};
use sqlx::{PgPool, Row};

pub struct ExplainPanel {
    focus_handle: FocusHandle,
    pool: PgPool,
    sql: String,
    plan_text: String,
    use_analyze: bool,
}

impl ExplainPanel {
    pub fn new(pool: PgPool, sql: String, _window: &mut Window, cx: &mut Context<Self>) -> Self {
        let mut p = Self {
            focus_handle: cx.focus_handle(),
            pool,
            sql,
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
            let text = match crate::db::run_infallible(cx, async move {
                let prefix = if analyze {
                    "EXPLAIN (ANALYZE, BUFFERS, FORMAT TEXT)"
                } else {
                    "EXPLAIN (FORMAT TEXT)"
                };
                let q = format!("{prefix} {stmt}");
                let rows = sqlx::query(&q).fetch_all(&pool).await.unwrap_or_default();
                let text = rows
                    .iter()
                    .filter_map(|row| row.try_get::<String, usize>(0).ok())
                    .collect::<Vec<_>>()
                    .join("\n");
                if text.is_empty() && rows.is_empty() {
                    String::from("(no plan rows)")
                } else if text.is_empty() {
                    "(could not read plan)".into()
                } else {
                    text
                }
            }).await {
                Ok(t) => t,
                Err(_) => "(error)".into(),
            };
            cx.update(|cx| {
                this.update(cx, |panel, cx| {
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

    fn closable(&self, _: &App) -> bool {
        true
    }

    fn title(&mut self, _: &mut Window, _cx: &mut Context<Self>) -> impl IntoElement {
        "EXPLAIN plan"
    }
}

impl Render for ExplainPanel {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let border = cx.theme().border;
        let body: SharedString = self.plan_text.clone().into();
        let analyze = self.use_analyze;

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
                    .overflow_y_scroll()
                    .p_3()
                    .font_family("monospace")
                    .text_sm()
                    .child(body),
            )
    }
}
