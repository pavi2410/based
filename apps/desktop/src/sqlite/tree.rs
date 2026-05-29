// sqlite::tree — SchemaTreePanel: displays tables/views from sqlite_master.

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
use sqlx::{Row, SqlitePool};

#[derive(Clone, Debug, PartialEq)]
pub enum ObjectKind {
    Table,
    View,
    Trigger,
}

impl ObjectKind {
    fn list_icon(&self) -> gpui_component::IconName {
        match self {
            Self::Table => gpui_component::IconName::LayoutDashboard,
            Self::View => gpui_component::IconName::Eye,
            Self::Trigger => gpui_component::IconName::TriangleAlert,
        }
    }
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
    pub(crate) tab_label: SharedString,
}

impl SchemaTreePanel {
    pub fn new(pool: SqlitePool, _window: &mut Window, cx: &mut Context<Self>) -> Self {
        let mut panel = Self {
            focus_handle: cx.focus_handle(),
            pool,
            nodes: vec![],
            selected: None,
            tab_label: "Schema".into(),
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

    crate::based_panel_tab_chrome!();

    fn title(&mut self, _: &mut Window, _cx: &mut Context<Self>) -> impl IntoElement {
        self.tab_label.clone()
    }
}

impl Render for SchemaTreePanel {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let nodes = self.nodes.clone();
        let selected = self.selected.clone();
        let muted = cx.theme().muted_foreground;
        let fg = cx.theme().foreground;
        let mono = cx.theme().mono_font_family.clone();
        let rows: Vec<_> = nodes
            .into_iter()
            .enumerate()
            .map(|(ix, node)| {
                let is_selected = selected.as_deref() == Some(&node.id());
                let name: SharedString = node.name.clone().into();
                let picked = node.name.clone();

                schema_object_row(
                    ("sqlite-obj", ix),
                    is_selected,
                    node.kind.list_icon(),
                    name,
                    SchemaRowStyle {
                        muted,
                        fg,
                        mono_family: mono.clone(),
                    },
                )
                .on_mouse_down(
                    MouseButton::Left,
                    cx.listener(move |panel, _, _window, cx| {
                        panel.selected = Some(picked.clone());
                        cx.emit(SchemaTreeEvent::TableSelected(picked.clone()));
                        cx.notify();
                    }),
                )
            })
            .collect();

        v_flex()
            .id("schema-tree-scroll")
            .w_full()
            .h_full()
            .min_h_0()
            .flex_1()
            .overflow_y_scroll()
            .bg(cx.theme().background)
            .child(panel_header(
                "SQLite Objects",
                "Tables, views, triggers, and virtual tables",
                cx,
            ))
            .child(
                h_flex()
                    .px_2()
                    .py_1()
                    .gap_2()
                    .border_b_1()
                    .border_color(cx.theme().border.opacity(0.72))
                    .bg(cx.theme().muted.opacity(0.18))
                    .child(metadata_pill("objects", rows.len().to_string(), cx))
                    .child(metadata_pill("engine", "SQLite", cx)),
            )
            .children(rows)
    }
}

impl TableNode {
    fn id(&self) -> String {
        self.name.clone()
    }
}
