use gpui::{
    App, Context, Entity, FocusHandle, Focusable, IntoElement, Render, SharedString, Window, div,
    prelude::*,
};
use gpui_component::{
    ActiveTheme,
    dock::{Panel, PanelControl, PanelEvent},
    menu::PopupMenu,
    scroll::ScrollableElement,
    v_flex,
};
use sqlx::SqlitePool;

use crate::based_panel_dropdown;
use crate::based_panel_tab_chrome;
use crate::connection::ConnectionId;
use crate::connection::{AnyConnection, ConnectionEntry, ConnectionState};
use crate::db;
use crate::query_store::QueryStore;
use crate::sqlite::pragmas::fetch_sqlite_pragmas;
use crate::widgets::PANEL_RADIUS;
use crate::widgets::{compact_description_list_vertical, engine_name, metadata_pill, panel_header};
use crate::workspace::tabs::render_strip_tab;

pub struct ConnectionDashboardPanel {
    focus_handle: FocusHandle,
    conn: Entity<ConnectionEntry>,
    pragmas: Vec<(SharedString, SharedString)>,
    loading_pragmas: bool,
}

impl ConnectionDashboardPanel {
    pub fn new(
        conn: Entity<ConnectionEntry>,
        _window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Self {
        cx.observe(&conn, |this, _, cx| {
            this.maybe_load_pragmas(cx);
            cx.notify();
        })
        .detach();

        let mut panel = Self {
            focus_handle: cx.focus_handle(),
            conn,
            pragmas: vec![],
            loading_pragmas: false,
        };
        panel.maybe_load_pragmas(cx);
        panel
    }

    pub(crate) fn connection_id(&self, cx: &App) -> ConnectionId {
        self.conn.read(cx).id.clone()
    }

    fn sqlite_pool(&self, cx: &App) -> Option<SqlitePool> {
        match &self.conn.read(cx).state {
            ConnectionState::Connected(AnyConnection::SQLite(ent)) => {
                Some(ent.read(cx).pool.clone())
            }
            _ => None,
        }
    }

    fn maybe_load_pragmas(&mut self, cx: &mut Context<Self>) {
        let Some(pool) = self.sqlite_pool(cx) else {
            self.pragmas.clear();
            self.loading_pragmas = false;
            return;
        };
        if self.loading_pragmas || !self.pragmas.is_empty() {
            return;
        }
        self.loading_pragmas = true;
        cx.spawn(async move |this, cx| {
            let rows = match db::run_infallible(
                cx,
                async move { fetch_sqlite_pragmas(&pool).await },
            )
            .await
            {
                Ok(rows) => rows,
                Err(_) => {
                    let _ = this.update(cx, |panel, _| {
                        panel.loading_pragmas = false;
                    });
                    return;
                }
            };
            let _ = this.update(cx, |panel, cx| {
                panel.pragmas = rows
                    .into_iter()
                    .map(|(k, v)| (k.into(), v.into()))
                    .collect();
                panel.loading_pragmas = false;
                cx.notify();
            });
        })
        .detach();
    }
}

impl gpui::EventEmitter<PanelEvent> for ConnectionDashboardPanel {}

impl Focusable for ConnectionDashboardPanel {
    fn focus_handle(&self, _: &App) -> FocusHandle {
        self.focus_handle.clone()
    }
}

impl Panel for ConnectionDashboardPanel {
    fn panel_name(&self) -> &'static str {
        "ConnectionDashboard"
    }

    fn dropdown_menu(
        &mut self,
        menu: PopupMenu,
        _window: &mut Window,
        cx: &mut Context<Self>,
    ) -> PopupMenu {
        based_panel_dropdown!(menu, self, cx)
    }

    fn tab_name(&self, _: &gpui::App) -> Option<gpui::SharedString> {
        None
    }

    fn title(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let label: SharedString = self.conn.read(cx).config.label().to_string().into();
        render_strip_tab(label, false, cx.entity().entity_id(), cx)
    }

    fn zoomable(&self, _: &gpui::App) -> Option<PanelControl> {
        None
    }

    fn closable(&self, _: &gpui::App) -> bool {
        false
    }
}

impl Render for ConnectionDashboardPanel {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let connected = {
            let entry = self.conn.read(cx);
            match &entry.state {
                ConnectionState::Connected(ac) => Some(ac.clone()),
                _ => None,
            }
        };

        let server_version = connected.and_then(|ac| match ac {
            AnyConnection::Postgres(e) => e.read(cx).server_version.clone(),
            AnyConnection::SQLite(e) => e.read(cx).server_version.clone(),
            AnyConnection::MongoDB(e) => e.read(cx).server_version.clone(),
        });

        let (engine, state_label, conn_id, read_only) = {
            let entry = self.conn.read(cx);
            (
                entry.config.engine(),
                entry.state.label(),
                entry.id.clone(),
                entry.config.is_read_only(),
            )
        };

        let store = cx.global::<QueryStore>();
        let history_len = store.history_for(&conn_id).len();
        let saved_len = store
            .project_queries()
            .iter()
            .filter(|q| {
                use based_project::TargetConnection;
                match &q.target.connection {
                    Some(TargetConnection::Exclusive(id)) => id == &conn_id.0,
                    Some(TargetConnection::OneOf(ids)) => ids.iter().any(|id| id == &conn_id.0),
                    None => false,
                }
            })
            .count();

        let query_rows: Vec<(SharedString, SharedString)> = vec![
            ("Session history".into(), history_len.to_string().into()),
            ("Saved queries".into(), saved_len.to_string().into()),
        ];

        let mut info_rows: Vec<(SharedString, SharedString)> = vec![
            ("Engine".into(), engine_name(engine).into()),
            ("State".into(), state_label.into()),
            ("Scope".into(), "local workspace".into()),
        ];
        if let Some(ref ver) = server_version {
            info_rows.push(("Server".into(), ver.clone().into()));
        }
        if read_only {
            info_rows.push(("Access".into(), "Read-only".into()));
        }

        let pragmas = self.pragmas.clone();

        v_flex()
            .size_full()
            .bg(cx.theme().background)
            .child(
                v_flex()
                    .flex_1()
                    .min_h_0()
                    .overflow_y_scrollbar()
                    .p_4()
                    .gap_4()
                    .child(compact_description_list_vertical(info_rows, true))
                    .child(dashboard_description_section("Queries", query_rows, cx))
                    .when(!pragmas.is_empty(), |this| {
                        this.child(dashboard_description_section("PRAGMAs", pragmas, cx))
                    })
                    .child(dashboard_card(
                        "Start",
                        "Use Catalog to open tables, views, or collections. Saved queries for this connection are in Queries. Use a query tab for ad-hoc work.",
                        cx,
                    ))
                    .child(dashboard_card(
                        "Workflow",
                        "Catalog and Queries stay in the sidebar; work opens as tabs in the center.",
                        cx,
                    )),
            )
    }
}

pub struct ObjectInfoPanel {
    focus_handle: FocusHandle,
    name: String,
    kind: String,
    pub(crate) tab_label: gpui::SharedString,
}

impl ObjectInfoPanel {
    pub fn new(
        name: String,
        kind: impl Into<String>,
        _window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Self {
        let kind = kind.into();
        let tab_label = format!("{name} ({kind})").into();
        Self {
            focus_handle: cx.focus_handle(),
            name,
            kind,
            tab_label,
        }
    }
}

impl gpui::EventEmitter<PanelEvent> for ObjectInfoPanel {}

impl Focusable for ObjectInfoPanel {
    fn focus_handle(&self, _: &App) -> FocusHandle {
        self.focus_handle.clone()
    }
}

impl Panel for ObjectInfoPanel {
    fn panel_name(&self) -> &'static str {
        "ObjectInfo"
    }

    fn dropdown_menu(
        &mut self,
        menu: PopupMenu,
        _window: &mut Window,
        cx: &mut Context<Self>,
    ) -> PopupMenu {
        based_panel_dropdown!(menu, self, cx)
    }

    based_panel_tab_chrome!();
}

impl Render for ObjectInfoPanel {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        v_flex()
            .size_full()
            .bg(cx.theme().background)
            .child(panel_header(
                self.name.clone(),
                format!("{} metadata", self.kind),
                cx,
            ))
            .child(
                v_flex()
                    .p_4()
                    .gap_3()
                    .child(metadata_pill("object", self.name.clone(), cx))
                    .child(metadata_pill("kind", self.kind.clone(), cx))
                    .child(dashboard_card(
                        "Inspector",
                        "Detailed schema, DDL, and dependency views will live here as the object model matures.",
                        cx,
                    )),
            )
    }
}

fn dashboard_description_section(
    title: impl Into<SharedString>,
    rows: impl IntoIterator<Item = (impl Into<SharedString>, impl Into<SharedString>)>,
    cx: &mut App,
) -> impl IntoElement {
    let title = title.into();
    v_flex()
        .gap_2()
        .child(
            div()
                .text_sm()
                .font_weight(gpui::FontWeight::SEMIBOLD)
                .text_color(cx.theme().foreground)
                .child(title),
        )
        .child(compact_description_list_vertical(rows, true))
}

fn dashboard_card(
    title: impl Into<SharedString>,
    body: impl Into<SharedString>,
    cx: &mut App,
) -> impl IntoElement {
    let title = title.into();
    let body = body.into();
    v_flex()
        .gap_1()
        .p_3()
        .w_full()
        .rounded(gpui::px(PANEL_RADIUS))
        .border_1()
        .border_color(cx.theme().border.opacity(0.85))
        .bg(cx.theme().muted.opacity(0.22))
        .child(
            div()
                .text_sm()
                .font_weight(gpui::FontWeight::SEMIBOLD)
                .text_color(cx.theme().foreground)
                .child(title),
        )
        .child(
            div()
                .text_sm()
                .text_color(cx.theme().muted_foreground)
                .child(body),
        )
}
