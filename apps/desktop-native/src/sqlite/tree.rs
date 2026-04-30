// sqlite::tree — SchemaTreePanel: displays tables/views from sqlite_master.

use gpui::{prelude::*, *};

use log::warn;
use gpui_component::{
    ActiveTheme,
    dock::{Panel, PanelEvent},
    menu::PopupMenu,
    h_flex, v_flex,
};
use sqlx::{Row, SqlitePool};

#[derive(Clone, Debug, PartialEq)]
pub enum ObjectKind {
    Table,
    View,
    Trigger,
}

#[derive(Clone, Debug)]
pub struct TableNode {
    pub name: String,
    pub kind: ObjectKind,
}

pub enum SchemaTreeEvent {
    TableSelected(String),
}

pub struct SchemaTreePanel {
    focus_handle: FocusHandle,
    pool: SqlitePool,
    nodes: Vec<TableNode>,
    selected: Option<String>,
}

impl SchemaTreePanel {
    pub fn new(pool: SqlitePool, _window: &mut Window, cx: &mut Context<Self>) -> Self {
        let mut panel = Self {
            focus_handle: cx.focus_handle(),
            pool,
            nodes: vec![],
            selected: None,
        };
        panel.load_tables(cx);
        panel
    }

    fn load_tables(&mut self, cx: &mut Context<Self>) {
        let pool = self.pool.clone();
        cx.spawn(async move |this, cx| {
            let nodes = crate::db::run(cx, async move {
                let rows = sqlx::query(
                    "SELECT name, type FROM sqlite_master \
                     WHERE type IN ('table','view','trigger') \
                     ORDER BY type, name",
                )
                .fetch_all(&pool)
                .await?;

                let nodes: Vec<TableNode> = rows
                    .iter()
                    .map(|row| {
                        let name: String = row.get("name");
                        let kind_str: String = row.get("type");
                        let kind = match kind_str.as_str() {
                            "view" => ObjectKind::View,
                            "trigger" => ObjectKind::Trigger,
                            _ => ObjectKind::Table,
                        };
                        TableNode { name, kind }
                    })
                    .collect();
                Ok(nodes)
            })
            .await;

            let nodes = match nodes {
                Ok(n) => n,
                Err(e) => {
                    warn!("sqlite schema load failed: {e:#}");
                    return;
                }
            };

            if this
                .update(cx, |panel, cx| {
                    panel.nodes = nodes;
                    cx.notify();
                })
                .is_err()
            {
                warn!("sqlite schema tree: entity update failed (panel released?)");
            }
        })
        .detach();
    }
}

impl EventEmitter<PanelEvent> for SchemaTreePanel {}
impl EventEmitter<SchemaTreeEvent> for SchemaTreePanel {}

impl Focusable for SchemaTreePanel {
    fn focus_handle(&self, _: &App) -> FocusHandle {
        self.focus_handle.clone()
    }
}

impl Panel for SchemaTreePanel {
    fn panel_name(&self) -> &'static str {
        "SqliteSchemaTree"
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
        "Schema"
    }
}

impl Render for SchemaTreePanel {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let nodes = self.nodes.clone();
        let selected = self.selected.clone();
        let accent = cx.theme().accent;

        let rows: Vec<_> = nodes
            .into_iter()
            .map(|node| {
                let is_selected = selected.as_deref() == Some(&node.id());
                let name: SharedString = node.name.clone().into();
                let kind_label: SharedString = match node.kind {
                    ObjectKind::Table => "T",
                    ObjectKind::View => "V",
                    ObjectKind::Trigger => "TR",
                }
                .into();

                h_flex()
                    .w_full()
                    .py(px(4.0))
                    .px(px(8.0))
                    .gap(px(6.0))
                    .cursor_pointer()
                    .when(is_selected, |d| d.bg(accent))
                    .on_mouse_down(
                        MouseButton::Left,
                        cx.listener(move |panel, _, _window, cx| {
                            panel.selected = Some(node.name.clone());
                            cx.emit(SchemaTreeEvent::TableSelected(node.name.clone()));
                            cx.notify();
                        }),
                    )
                    .child(
                        div()
                            .text_xs()
                            .opacity(0.5)
                            .w(px(16.0))
                            .child(kind_label),
                    )
                    .child(div().text_sm().child(name))
            })
            .collect();

        v_flex()
            .id("schema-tree-scroll")
            .w_full()
            .h_full()
            .min_h_0()
            .flex_1()
            .overflow_y_scroll()
            .children(rows)
    }
}

impl TableNode {
    fn id(&self) -> String {
        self.name.clone()
    }
}
