// Tabbed table inspector (columns, indexes, ...) shared by SQL engines via [`InspectSource`].

use std::sync::Arc;

use futures::future::BoxFuture;
use gpui_kit::component::{
    ActiveTheme,
    button::Button,
    dock::{BasePanel, Panel, PanelEvent},
    h_flex,
    input::EditorState,
    menu::PopupMenu,
    table::{Column, TableState},
    v_flex,
};
use gpui_kit::{prelude::*, *};

use based_core::{Grid, TableExtra, TableInfo};

use crate::connection::ConnectionId;
use crate::db;
use crate::widgets::compact_description_list_vertical;
use crate::widgets::data_table::{configure_row_table, render_row_table};
use crate::widgets::panel::{
    panel_tab_content, tab_breadcrumb_footer, tab_breadcrumb_for_connection, tab_button_styled,
};
use crate::widgets::sql_editor::{self, new_sql_input, set_input_text};
use crate::widgets::virtual_table::{
    RowDelegate, data_column, empty_column_meta, replace_table_data,
};

#[derive(Clone, Copy)]
pub enum Kind {
    Table,
    Code,
    Facts,
}

/// A tab the source wants shown; `load` returns one [`Section`] per tab, in order.
#[derive(Clone, Copy)]
pub struct TabDef {
    pub id: &'static str,
    pub label: &'static str,
    pub kind: Kind,
}

pub enum Section {
    Table {
        columns: Vec<Column>,
        rows: Vec<Vec<SharedString>>,
    },
    Code(String),
    Facts(Vec<(SharedString, SharedString)>),
}

impl Section {
    /// Columns, Indexes, Constraints, then the engine extra (DDL or stats).
    pub fn from_info(info: TableInfo) -> Vec<Section> {
        let grid = |g: Grid| Section::Table {
            columns: g.headers.into_iter().map(|h| data_column(h, h)).collect(),
            rows: g
                .rows
                .into_iter()
                .map(|r| r.into_iter().map(SharedString::from).collect())
                .collect(),
        };
        let extra = match info.extra {
            TableExtra::Ddl(ddl) => Section::Code(ddl),
            TableExtra::Stats(kv) => {
                Section::Facts(kv.into_iter().map(|(k, v)| (k.into(), v.into())).collect())
            }
        };
        vec![
            grid(info.columns),
            grid(info.indexes),
            grid(info.constraints),
            extra,
        ]
    }
}

/// Engine-specific schema queries for [`TableInspectorPanel`].
pub trait InspectSource: Send + Sync + 'static {
    /// Dock panel name, e.g. `"PgTableInspector"`.
    fn panel_name(&self) -> &'static str;
    /// Breadcrumb id, e.g. `"pg-insp-breadcrumb"`.
    fn breadcrumb_id(&self) -> &'static str;
    /// Object path shown in breadcrumbs and labels, e.g. `["public", "users"]`.
    fn path(&self) -> Vec<String>;
    fn tabs(&self) -> Vec<TabDef>;
    fn load(&self) -> BoxFuture<'static, anyhow::Result<Vec<Section>>>;
}

enum Body {
    Table(Entity<TableState<RowDelegate>>),
    Code(Entity<EditorState>),
    Facts(Vec<(SharedString, SharedString)>),
}

pub struct TableInspectorPanel {
    focus_handle: FocusHandle,
    source: Arc<dyn InspectSource>,
    conn_id: ConnectionId,
    tabs: Vec<(TabDef, Body)>,
    active: usize,
    pub(crate) tab_label: SharedString,
}

impl TableInspectorPanel {
    pub fn new(
        source: Arc<dyn InspectSource>,
        conn_id: ConnectionId,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Self {
        let tabs = source
            .tabs()
            .into_iter()
            .map(|def| {
                let body = match def.kind {
                    Kind::Table => Body::Table(
                        cx.new(|cx| configure_row_table(RowDelegate::default(), window, cx)),
                    ),
                    Kind::Code => Body::Code(new_sql_input("(loading…)", window, cx)),
                    Kind::Facts => Body::Facts(vec![]),
                };
                (def, body)
            })
            .collect();
        let tab_label = format!("{} (schema)", source.path().join(".")).into();
        let mut panel = Self {
            focus_handle: cx.focus_handle(),
            source,
            conn_id,
            tabs,
            active: 0,
            tab_label,
        };
        panel.load(cx);
        panel
    }

    fn load(&mut self, cx: &mut Context<Self>) {
        let fetch = self.source.load();
        cx.spawn(async move |this, cx| {
            let Ok(sections) = db::run(cx, fetch).await else {
                return;
            };
            cx.update(|cx| {
                let mut code = vec![];
                let _ = this.update(cx, |panel, cx| {
                    for ((_, body), section) in panel.tabs.iter_mut().zip(sections) {
                        match (body, section) {
                            (Body::Table(ent), Section::Table { columns, rows }) => {
                                let meta = empty_column_meta(columns.len());
                                ent.update(cx, |state, cx| {
                                    replace_table_data(state, columns, rows, meta, cx);
                                });
                            }
                            (Body::Code(ent), Section::Code(text)) => {
                                code.push((ent.clone(), text))
                            }
                            (Body::Facts(slot), Section::Facts(facts)) => *slot = facts,
                            _ => {}
                        }
                    }
                    cx.notify();
                });
                if let Some(handle) = cx.active_window() {
                    let _ = handle.update(cx, |_root, window, cx| {
                        for (input, text) in &code {
                            set_input_text(input, text, window, cx);
                        }
                    });
                }
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

impl BasePanel for TableInspectorPanel {
    fn panel_name(&self) -> &'static str {
        self.source.panel_name()
    }

    fn closable(&self, _: &App) -> bool {
        false
    }

    fn zoomable(&self, _: &App) -> bool {
        false
    }
}

impl Panel for TableInspectorPanel {
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

        let buttons: Vec<Button> = self
            .tabs
            .iter()
            .enumerate()
            .map(|(i, (def, _))| {
                tab_button_styled(def.id, def.label, i == self.active).on_click(cx.listener(
                    move |panel, _, _, cx| {
                        panel.active = i;
                        cx.notify();
                    },
                ))
            })
            .collect();

        let content: AnyElement = match self.tabs.get(self.active).map(|(_, b)| b) {
            Some(Body::Table(ent)) => render_row_table(ent, cx).into_any_element(),
            Some(Body::Code(ent)) => div()
                .id("inspector-code")
                .size_full()
                .min_h_0()
                .child(sql_editor::code_editor_flex(ent, false, true, cx))
                .into_any_element(),
            Some(Body::Facts(facts)) => div()
                .p_3()
                .child(compact_description_list_vertical(facts.clone(), true))
                .into_any_element(),
            None => div().into_any_element(),
        };

        let body = v_flex()
            .size_full()
            .gap_2()
            .child(
                h_flex()
                    .w_full()
                    .justify_center()
                    .gap_2()
                    .py_2()
                    .px_2()
                    .border_b_1()
                    .border_color(border)
                    .children(buttons),
            )
            .child(
                div()
                    .flex_1()
                    .min_h(px(200.0))
                    .border_1()
                    .border_color(border)
                    .child(content),
            );

        let crumbs = tab_breadcrumb_for_connection(&self.conn_id, self.source.path(), cx);
        let footer = tab_breadcrumb_footer(self.source.breadcrumb_id(), crumbs, None, cx);

        panel_tab_content(body, footer).into_any_element()
    }
}
