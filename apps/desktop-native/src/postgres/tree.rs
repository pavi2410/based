// postgres::tree — schema-qualified relation list from information_schema.

use gpui::{prelude::*, *};
use gpui_component::{
    ActiveTheme,
    dock::{Panel, PanelEvent},
    h_flex, v_flex,
};
use sqlx::{PgPool, Row};

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum RelKind {
    Table,
    View,
    Matview,
}

#[derive(Clone, Debug)]
pub struct RelRef {
    pub schema: String,
    pub name: String,
    pub kind: RelKind,
}

pub enum PgSchemaTreeEvent {
    RelationSelected(RelRef),
}

pub struct SchemaTreePanel {
    focus_handle: FocusHandle,
    pool: PgPool,
    nodes: Vec<RelRef>,
    selected: Option<(String, String)>,
}

impl SchemaTreePanel {
    pub fn new(pool: PgPool, _window: &mut Window, cx: &mut Context<Self>) -> Self {
        let mut panel = Self {
            focus_handle: cx.focus_handle(),
            pool,
            nodes: vec![],
            selected: None,
        };
        panel.load_relations(cx);
        panel
    }

    fn load_relations(&mut self, cx: &mut Context<Self>) {
        let pool = self.pool.clone();
        cx.spawn(async move |this, cx| {
            let rows = crate::tokio_bridge::block_on_db(async move {
                sqlx::query(
                    r"SELECT table_schema, table_name, table_type
                  FROM information_schema.tables
                  WHERE table_schema NOT IN ('pg_catalog', 'information_schema')
                  ORDER BY table_schema, table_name",
                )
                .fetch_all(&pool)
                .await
            });
            let rows = match rows {
                Ok(r) => r,
                Err(_) => return,
            };

            let nodes: Vec<RelRef> = rows
                .iter()
                .map(|row| {
                    let schema: String = row.get("table_schema");
                    let name: String = row.get("table_name");
                    let ty: String = row.get("table_type");
                    let kind = match ty.as_str() {
                        "VIEW" => RelKind::View,
                        "MATERIALIZED VIEW" => RelKind::Matview,
                        _ => RelKind::Table,
                    };
                    RelRef {
                        schema,
                        name,
                        kind,
                    }
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

    fn badge(kind: &RelKind) -> &'static str {
        match kind {
            RelKind::Table => "tbl",
            RelKind::View => "vw",
            RelKind::Matview => "mv",
        }
    }
}

impl EventEmitter<PanelEvent> for SchemaTreePanel {}
impl EventEmitter<PgSchemaTreeEvent> for SchemaTreePanel {}

impl Focusable for SchemaTreePanel {
    fn focus_handle(&self, _: &App) -> FocusHandle {
        self.focus_handle.clone()
    }
}

impl Panel for SchemaTreePanel {
    fn panel_name(&self) -> &'static str {
        "PgSchemaTree"
    }

    fn closable(&self, _: &App) -> bool {
        false
    }

    fn title(&mut self, _: &mut Window, _cx: &mut Context<Self>) -> impl IntoElement {
        "PostgreSQL objects"
    }
}

impl Render for SchemaTreePanel {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let border = cx.theme().border;
        let fg = cx.theme().foreground;
        let muted = cx.theme().muted_foreground;

        let rows: Vec<RelRef> = self.nodes.clone();
        let selected = self.selected.clone();

        let list = rows
            .into_iter()
            .enumerate()
            .map(|(ix, rel)| {
                let key = (rel.schema.clone(), rel.name.clone());
                let is_sel = selected.as_ref() == Some(&key);
                let badge = Self::badge(&rel.kind);
                let label2: SharedString = format!("{}.{}", rel.schema, rel.name).into();
                let picked = rel.clone();

                h_flex()
                    .id(("pg-rel", ix))
                    .px(px(8.0))
                    .py(px(4.0))
                    .gap(px(6.0))
                    .cursor_pointer()
                    .when(is_sel, |d| d.bg(cx.theme().accent.opacity(0.12)))
                    .on_click(cx.listener(move |panel, _, _, cx| {
                        panel.selected = Some(key.clone());
                        cx.emit(PgSchemaTreeEvent::RelationSelected(picked.clone()));
                        cx.notify();
                    }))
                    .child(
                        div()
                            .text_xs()
                            .text_color(muted)
                            .w(px(28.0))
                            .child(badge),
                    )
                    .child(div().text_sm().text_color(fg).child(label2))
            })
            .collect::<Vec<_>>();

        v_flex()
            .id("pg-schema-tree-scroll")
            .w_full()
            .h_full()
            .overflow_y_scroll()
            .child(
                div()
                    .px(px(8.0))
                    .py(px(6.0))
                    .border_b_1()
                    .border_color(border)
                    .text_sm()
                    .font_weight(FontWeight::SEMIBOLD)
                    .child("Schemas & tables"),
            )
            .children(list)
    }
}
