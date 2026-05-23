// postgres::tree — schema-qualified relation list from information_schema.

use gpui::{InteractiveElement, prelude::*, *};

use crate::widgets::list_row::{SchemaRowStyle, schema_object_row};
use crate::widgets::ui::{metadata_pill, panel_header};
use gpui_component::{
    ActiveTheme,
    dock::{Panel, PanelEvent},
    h_flex,
    menu::PopupMenu,
    v_flex,
};
use log::warn;
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
            let rows = match crate::db::run(cx, async move {
                Ok(sqlx::query(
                    r"SELECT table_schema, table_name, table_type
                  FROM information_schema.tables
                  WHERE table_schema NOT IN ('pg_catalog', 'information_schema')
                  ORDER BY table_schema, table_name",
                )
                .fetch_all(&pool)
                .await?)
            })
            .await
            {
                Ok(r) => r,
                Err(e) => {
                    warn!("postgres schema load failed: {e:#}");
                    return;
                }
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
                    RelRef { schema, name, kind }
                })
                .collect();

            if this
                .update(cx, |panel, cx| {
                    panel.nodes = nodes;
                    cx.notify();
                })
                .is_err()
            {
                warn!("postgres schema tree: entity update failed (panel released?)");
            }
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

    fn dropdown_menu(
        &mut self,
        menu: PopupMenu,
        _window: &mut Window,
        cx: &mut Context<Self>,
    ) -> PopupMenu {
        crate::based_panel_dropdown!(menu, self, cx)
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
        let rows: Vec<RelRef> = self.nodes.clone();
        let selected = self.selected.clone();
        let muted = cx.theme().muted_foreground;
        let fg = cx.theme().foreground;
        let mono = cx.theme().mono_font_family.clone();

        let list = rows
            .into_iter()
            .enumerate()
            .map(|(ix, rel)| {
                let key = (rel.schema.clone(), rel.name.clone());
                let is_sel = selected.as_ref() == Some(&key);
                let badge = Self::badge(&rel.kind);
                let label2: SharedString = format!("{}.{}", rel.schema, rel.name).into();
                let picked = rel.clone();

                schema_object_row(
                    ("pg-rel", ix),
                    is_sel,
                    badge,
                    label2,
                    SchemaRowStyle {
                        muted,
                        fg,
                        mono_family: mono.clone(),
                    },
                )
                .on_click(cx.listener(move |panel, _, _, cx| {
                    panel.selected = Some(key.clone());
                    cx.emit(PgSchemaTreeEvent::RelationSelected(picked.clone()));
                    cx.notify();
                }))
            })
            .collect::<Vec<_>>();

        v_flex()
            .id("pg-schema-tree-scroll")
            .w_full()
            .h_full()
            .min_h_0()
            .flex_1()
            .overflow_y_scroll()
            .bg(cx.theme().background)
            .child(panel_header(
                "Postgres Objects",
                "Schemas, tables, views, and materialized views",
                cx,
            ))
            .child(
                h_flex()
                    .px_2()
                    .py_1()
                    .gap_2()
                    .border_b_1()
                    .border_color(border.opacity(0.72))
                    .bg(cx.theme().muted.opacity(0.18))
                    .child(metadata_pill("relations", list.len().to_string(), cx))
                    .child(metadata_pill("engine", "Postgres", cx)),
            )
            .children(list)
    }
}
