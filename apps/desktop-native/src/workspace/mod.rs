// workspace/ — Workspace entity, DockArea, sidebar, status bar, connection wiring.

pub mod item;
pub mod notify;
pub mod pane;
pub mod pop_out;
pub use pop_out::PopOutManager;
pub mod sidebar;
pub mod status_bar;
pub mod topbar;
pub mod welcome;

use std::sync::Arc;
use std::time::Instant;

use gpui::{
    Context, Entity, FocusHandle, Focusable, FontWeight, IntoElement, Render, SharedString, Window,
    div, prelude::*,
};
use gpui_component::{
    ActiveTheme,
    Icon, IconName,
    Sizable as _,
    StyledExt,
    dock::{DockArea, DockItem, PanelStyle},
    h_flex, v_flex,
    tooltip::Tooltip,
};
use crate::widgets::ui::{engine_chip, engine_color, engine_name, metadata_pill};

use crate::bindings::{CycleAppearance, ToggleSidebarRail};
use crate::connection::lifecycle::Connectable;
use crate::connection::registry::ConnectionRegistry;
use crate::connection::{
    AnyConnection, ConnectionConfig, ConnectionEntry, ConnectionState,
};
use crate::mongodb::MongoConnection;
use crate::postgres;
use crate::project::{find_project_root, load_workspace_seed};
use crate::sqlite::{self, SqliteConnection};
use ::mongodb::bson::Document;

use status_bar::StatusBar;
use topbar::Topbar;
use welcome::WelcomePanel;

pub struct Workspace {
    registry: Entity<ConnectionRegistry>,
    dock_area: Entity<DockArea>,
    sidebar_collapsed: bool,
    inspector_collapsed: bool,
    selected_connection: Option<usize>,
    focus_handle: FocusHandle,
    /// After async connect, open engine tabs on next render (needs `&mut Window`).
    pending_open_connection: Option<usize>,
    project_title: SharedString,
}

impl Workspace {
    pub fn new(window: &mut Window, cx: &mut Context<Self>) -> Self {
        let (project_title, entries) = find_project_root()
            .map(|root| {
                let (title, e) = load_workspace_seed(&root);
                (title.into(), e)
            })
            .unwrap_or_else(|| ("No Project".into(), vec![]));

        if entries.is_empty() {
            log::warn!(
                "no connections loaded; add [connection.id] tables to .based/config.toml (or set BASED_PROJECT_DIR)"
            );
        }

        let registry = cx.new(ConnectionRegistry::new);
        registry.update(cx, |reg, cx| {
            for entry in entries {
                reg.add(entry, cx);
            }
        });

        let dock_area = cx.new(|cx| {
            DockArea::new("workspace", Some(1), window, cx).panel_style(PanelStyle::TabBar)
        });

        let welcome = cx.new(|cx| WelcomePanel::new(window, cx));
        let center = DockItem::tab(welcome, &dock_area.downgrade(), window, cx);
        dock_area.update(cx, |area, cx| {
            area.set_center(center, window, cx);
        });

        let workspace = Self {
            registry: registry.clone(),
            dock_area,
            sidebar_collapsed: crate::app::prefs::collapsed_from(cx),
            inspector_collapsed: false,
            selected_connection: None,
            focus_handle: cx.focus_handle(),
            pending_open_connection: None,
            project_title,
        };

        let _ = cx.observe(&registry, |_ws, _reg, cx| {
            cx.notify();
        });

        workspace
    }

    pub fn toggle_sidebar_rail(&mut self, cx: &mut Context<Self>) {
        self.sidebar_collapsed = !self.sidebar_collapsed;
        crate::app::prefs::set_sidebar(self.sidebar_collapsed, cx);
        cx.notify();
    }

    fn on_connection_row_clicked(
        &mut self,
        idx: usize,
        _window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.selected_connection = Some(idx);
        let conn_ent = match self.registry.read(cx).connections().get(idx) {
            Some(e) => e.clone(),
            None => return,
        };

        match conn_ent.read(cx).state {
            ConnectionState::Connecting { .. } => return,
            ConnectionState::Connected(_) => {
                self.pending_open_connection = Some(idx);
                cx.notify();
                return;
            }
            ConnectionState::Disconnected | ConnectionState::Failed { .. } => {}
        }

        let config = conn_ent.read(cx).config.clone();
        conn_ent.update(cx, |e, cx| {
            e.state = ConnectionState::Connecting {
                since: Instant::now(),
            };
            e.last_error = None;
            cx.notify();
        });
        cx.notify();

        let workspace = cx.entity().clone();
        let idx_for_pending = idx;
        let conn_label = config.label().to_string();
        let conn_engine = config.engine();

        match config {
            ConnectionConfig::SQLite(cfg) => {
                let task = SqliteConnection::open(cfg, cx);
                cx.spawn(async move |_, cx| {
                    let result = task.await;
                    let _ = cx.update(|app| {
                        let mut tray_fail: Option<(String, String, String)> = None;
                        conn_ent.update(app, |entry, ecx| {
                            match result {
                                Ok(conn) => {
                                    let ent = ecx.new(|_| conn);
                                    entry.state =
                                        ConnectionState::Connected(AnyConnection::SQLite(ent));
                                }
                                Err(err) => {
                                    log::warn!(
                                        "connection failed: label=\"{}\" engine={} error={:#}",
                                        conn_label,
                                        conn_engine.short_label(),
                                        err
                                    );
                                    entry.state = ConnectionState::Failed {
                                        reason: err.to_string(),
                                        attempted_at: Instant::now(),
                                    };
                                    entry.last_error = Some(err.to_string());
                                    tray_fail = Some((
                                        conn_label.clone(),
                                        conn_engine.short_label().to_string(),
                                        format!("{err:#}"),
                                    ));
                                }
                            }
                            ecx.notify();
                        });
                        if let Some((l, e, d)) = tray_fail {
                            notify::push_connection_failure(app, l, e, d);
                        }
                        workspace.update(app, |ws, ecx| {
                            if matches!(conn_ent.read(ecx).state, ConnectionState::Connected(_)) {
                                ws.pending_open_connection = Some(idx_for_pending);
                            }
                            ecx.notify();
                        });
                    });
                })
                .detach();
            }
            ConnectionConfig::Postgres(cfg) => {
                let task = postgres::PgConnection::open(cfg, cx);
                cx.spawn(async move |_, cx| {
                    let result = task.await;
                    let _ = cx.update(|app| {
                        let mut tray_fail: Option<(String, String, String)> = None;
                        conn_ent.update(app, |entry, ecx| {
                            match result {
                                Ok(conn) => {
                                    let ent = ecx.new(|_| conn);
                                    entry.state =
                                        ConnectionState::Connected(AnyConnection::Postgres(ent));
                                }
                                Err(err) => {
                                    log::warn!(
                                        "connection failed: label=\"{}\" engine={} error={:#}",
                                        conn_label,
                                        conn_engine.short_label(),
                                        err
                                    );
                                    entry.state = ConnectionState::Failed {
                                        reason: err.to_string(),
                                        attempted_at: Instant::now(),
                                    };
                                    entry.last_error = Some(err.to_string());
                                    tray_fail = Some((
                                        conn_label.clone(),
                                        conn_engine.short_label().to_string(),
                                        format!("{err:#}"),
                                    ));
                                }
                            }
                            ecx.notify();
                        });
                        if let Some((l, e, d)) = tray_fail {
                            notify::push_connection_failure(app, l, e, d);
                        }
                        workspace.update(app, |ws, ecx| {
                            if matches!(conn_ent.read(ecx).state, ConnectionState::Connected(_)) {
                                ws.pending_open_connection = Some(idx_for_pending);
                            }
                            ecx.notify();
                        });
                    });
                })
                .detach();
            }
            ConnectionConfig::MongoDB(cfg) => {
                let task = MongoConnection::open(cfg, cx);
                cx.spawn(async move |_, cx| {
                    let result = task.await;
                    let _ = cx.update(|app| {
                        let mut tray_fail: Option<(String, String, String)> = None;
                        conn_ent.update(app, |entry, ecx| {
                            match result {
                                Ok(conn) => {
                                    let ent = ecx.new(|_| conn);
                                    entry.state =
                                        ConnectionState::Connected(AnyConnection::MongoDB(ent));
                                }
                                Err(err) => {
                                    log::warn!(
                                        "connection failed: label=\"{}\" engine={} error={:#}",
                                        conn_label,
                                        conn_engine.short_label(),
                                        err
                                    );
                                    entry.state = ConnectionState::Failed {
                                        reason: err.to_string(),
                                        attempted_at: Instant::now(),
                                    };
                                    entry.last_error = Some(err.to_string());
                                    tray_fail = Some((
                                        conn_label.clone(),
                                        conn_engine.short_label().to_string(),
                                        format!("{err:#}"),
                                    ));
                                }
                            }
                            ecx.notify();
                        });
                        if let Some((l, e, d)) = tray_fail {
                            notify::push_connection_failure(app, l, e, d);
                        }
                        workspace.update(app, |ws, ecx| {
                            if matches!(conn_ent.read(ecx).state, ConnectionState::Connected(_)) {
                                ws.pending_open_connection = Some(idx_for_pending);
                            }
                            ecx.notify();
                        });
                    });
                })
                .detach();
            }
        }
    }

    fn open_connected_tabs(
        &mut self,
        ac: &AnyConnection,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let weak = self.dock_area.downgrade();
        let center = match ac {
            AnyConnection::SQLite(ent) => {
                let pool = ent.read(cx).pool.clone();
                let tree = cx.new(|cx| sqlite::tree::SchemaTreePanel::new(pool.clone(), window, cx));
                let query =
                    cx.new(|cx| sqlite::query_editor::QueryEditorPanel::new(pool.clone(), window, cx));
                let pragma =
                    cx.new(|cx| sqlite::pragma_browser::PragmaBrowserPanel::new(pool.clone(), window, cx));
                DockItem::tabs(
                    vec![Arc::new(tree), Arc::new(query), Arc::new(pragma)],
                    &weak,
                    window,
                    cx,
                )
            }
            AnyConnection::Postgres(ent) => {
                let pool = ent.read(cx).pool.clone();
                let tree =
                    cx.new(|cx| postgres::tree::SchemaTreePanel::new(pool.clone(), window, cx));
                let query =
                    cx.new(|cx| postgres::query_editor::QueryEditorPanel::new(pool.clone(), window, cx));
                let monitor =
                    cx.new(|cx| postgres::live_monitor::LiveMonitorPanel::new(pool.clone(), window, cx));
                DockItem::tabs(
                    vec![Arc::new(tree), Arc::new(query), Arc::new(monitor)],
                    &weak,
                    window,
                    cx,
                )
            }
            AnyConnection::MongoDB(ent) => {
                let db = ent.read(cx).database().clone();
                let coll: ::mongodb::Collection<Document> = db.collection("based_explorer");
                let tree =
                    cx.new(|cx| crate::mongodb::tree::CollectionsTreePanel::new(db.clone(), window, cx));
                let builder =
                    cx.new(|cx| crate::mongodb::pipeline_builder::PipelineBuilderPanel::new(coll.clone(), window, cx));
                let stream =
                    cx.new(|cx| crate::mongodb::change_stream::ChangeStreamPanel::new(coll, window, cx));
                DockItem::tabs(
                    vec![Arc::new(tree), Arc::new(builder), Arc::new(stream)],
                    &weak,
                    window,
                    cx,
                )
            }
        };

        self.dock_area.update(cx, |dock, cx| {
            dock.set_center(center, window, cx);
        });
    }
}

impl Focusable for Workspace {
    fn focus_handle(&self, _: &gpui::App) -> FocusHandle {
        self.focus_handle.clone()
    }
}

impl Render for Workspace {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        if let Some(idx) = self.pending_open_connection.take() {
            let ac = if let Some(ent) = self.registry.read(cx).connections().get(idx) {
                match &ent.read(cx).state {
                    ConnectionState::Connected(ac) => Some(ac.clone()),
                    _ => None,
                }
            } else {
                None
            };
            if let Some(ac) = ac {
                self.open_connected_tabs(&ac, window, cx);
            }
        }

        let this = cx.entity().clone();
        let conn_list: Vec<Entity<ConnectionEntry>> =
            self.registry.read(cx).connections().to_vec();
        let conn_count = conn_list.len();
        let connected_count = conn_list
            .iter()
            .filter(|ent| matches!(ent.read(cx).state, ConnectionState::Connected(_)))
            .count();
        let border = cx.theme().sidebar_border;
        let sidebar_bg = cx.theme().sidebar;
        let muted = cx.theme().muted_foreground;
        let sfg = cx.theme().sidebar_foreground;
        let list_hover = cx.theme().list_hover;

        let sidebar = v_flex()
            .w(gpui::px(274.0))
            .h_full()
            .flex_shrink_0()
            .border_r_1()
            .border_color(border)
            .bg(sidebar_bg)
            .child(
                h_flex()
                    .h(gpui::px(38.0))
                    .px_2()
                    .gap_2()
                    .items_center()
                    .border_b_1()
                    .border_color(border.opacity(0.86))
                    .child(
                        v_flex()
                            .flex_1()
                            .min_w_0()
                            .child(
                                div()
                                    .text_xs()
                                    .font_bold()
                                    .text_color(muted)
                                    .font_family(cx.theme().mono_font_family.clone())
                                    .child("WORKSPACE"),
                            )
                            .child(
                                div()
                                    .text_xs()
                                    .text_color(muted.opacity(0.82))
                                    .truncate()
                                    .child(format!("{conn_count} connections · {connected_count} live")),
                            ),
                    )
                    .child(
                        div()
                            .h(gpui::px(22.0))
                            .px(gpui::px(7.0))
                            .rounded(gpui::px(5.0))
                            .border_1()
                            .border_color(border.opacity(0.8))
                            .bg(cx.theme().muted.opacity(0.38))
                            .text_xs()
                            .text_color(muted)
                            .flex()
                            .items_center()
                            .child("+"),
                    ),
            )
            .child(
                h_flex()
                    .mx_2()
                    .my_2()
                    .h(gpui::px(28.0))
                    .items_center()
                    .gap_2()
                    .px_2()
                    .rounded(gpui::px(6.0))
                    .border_1()
                    .border_color(border.opacity(0.78))
                    .bg(cx.theme().muted.opacity(0.32))
                    .child(Icon::new(IconName::Search).with_size(gpui_component::Size::XSmall).text_color(muted))
                    .child(
                        div()
                            .text_xs()
                            .text_color(muted)
                            .truncate()
                            .child("Search tables, queries, collections"),
                    ),
            )
            .child(
                div()
                    .px_2()
                    .pb_1()
                    .text_xs()
                    .font_bold()
                    .font_family(cx.theme().mono_font_family.clone())
                    .text_color(muted.opacity(0.86))
                    .child("CONNECTIONS"),
            )
            .children(conn_list.into_iter().enumerate().map(|(idx, ent)| {
                let entry = ent.read(cx);
                let state_color = connection_state_dot(&entry.state, cx.theme());
                let engine = entry.config.engine();
                let conn_label = entry.config.label().to_string();
                let state_label = entry.state.label();
                let is_selected = self.selected_connection == Some(idx);
                let is_failed = matches!(entry.state, ConnectionState::Failed { .. });
                let fail_reason = match &entry.state {
                    ConnectionState::Failed { reason, .. } => Some(reason.clone()),
                    _ => None,
                };
                let err_fg = cx.theme().danger_foreground;

                let status_cell = if is_failed {
                    h_flex()
                        .flex_shrink_0()
                        .gap_1()
                        .items_center()
                        .child(
                            Icon::new(IconName::CircleX)
                                .text_color(err_fg)
                                .with_size(gpui_component::Size::XSmall),
                        )
                        .child(
                            div()
                                .text_xs()
                                .font_semibold()
                                .text_color(err_fg)
                                .child("Failed"),
                        )
                } else {
                    h_flex()
                        .flex_shrink_0()
                        .child(
                            div()
                                .text_xs()
                                .text_color(muted)
                                .child(state_label),
                        )
                };

                let main_row = h_flex()
                    .w_full()
                    .gap_2()
                    .items_center()
                    .child(
                        div()
                            .w_2()
                            .h_2()
                            .rounded_full()
                            .flex_shrink_0()
                            .bg(state_color),
                    )
                    .child(
                        v_flex()
                            .flex_1()
                            .min_w_0()
                            .gap(gpui::px(1.0))
                            .child(
                                div()
                                    .text_sm()
                                    .text_color(sfg)
                                    .truncate()
                                    .when(is_failed, |d| d.text_color(err_fg.opacity(0.92)))
                                    .child(conn_label.clone()),
                            )
                            .child(
                                h_flex()
                                    .gap_1()
                                    .items_center()
                                    .child(engine_chip(engine, cx))
                                    .child(
                                        div()
                                            .text_xs()
                                            .text_color(muted.opacity(0.82))
                                            .child("local"),
                                    ),
                            ),
                    )
                    .child(status_cell);

                let mut row = main_row.id(("conn-row", idx));
                if let Some(reason) = fail_reason {
                    let reason_tip: SharedString = reason.clone().into();
                    row = row.tooltip(move |window, app| {
                        Tooltip::element({
                            let reason_tip = reason_tip.clone();
                            move |_w, tip_cx| {
                                let fg = tip_cx.theme().foreground;
                                let subtle = tip_cx.theme().muted_foreground;
                                v_flex()
                                    .gap_1()
                                    .max_w(gpui::px(400.0))
                                    .child(
                                        div()
                                            .text_xs()
                                            .font_semibold()
                                            .text_color(fg)
                                            .child("Could not connect"),
                                    )
                                    .child(
                                        div()
                                            .text_xs()
                                            .text_color(subtle)
                                            .font_family(tip_cx.theme().mono_font_family.clone())
                                            .child(reason_tip.clone()),
                                    )
                            }
                        })
                        .build(window, app)
                    });
                }

                row
                    .px_2()
                    .py_1()
                    .cursor_pointer()
                    .rounded(gpui::px(7.0))
                    .mx_2()
                    .mb(gpui::px(2.0))
                    .when(is_selected, |r| {
                        r.bg(cx.theme().accent.opacity(0.22))
                            .border_1()
                            .border_color(engine_color(engine).opacity(0.26))
                    })
                    .when(is_failed, |r| {
                        r.border_1()
                            .border_color(cx.theme().danger.opacity(0.35))
                    })
                    .hover(move |s| s.bg(list_hover))
                    .on_click(cx.listener(move |ws, _, window, cx| {
                        ws.on_connection_row_clicked(idx, window, cx);
                    }))
            }));

        let dock_host = div()
            .flex_1()
            .size_full()
            .overflow_hidden()
            .child(self.dock_area.clone());

        let selected_connection = self
            .selected_connection
            .and_then(|idx| self.registry.read(cx).connections().get(idx).cloned());
        let inspector = render_inspector(selected_connection, cx);

        let body = if self.sidebar_collapsed {
            h_flex()
                .flex_1()
                .overflow_hidden()
                .child(dock_host)
                .when(!self.inspector_collapsed, |row| row.child(inspector))
        } else {
            h_flex()
                .flex_1()
                .overflow_hidden()
                .child(sidebar)
                .child(dock_host)
                .when(!self.inspector_collapsed, |row| row.child(inspector))
        };

        v_flex()
            .size_full()
            .track_focus(&self.focus_handle)
            .on_action(window.listener_for(&this, |ws, _: &ToggleSidebarRail, _, cx| {
                ws.toggle_sidebar_rail(cx);
            }))
            .on_action(window.listener_for(&this, |_, _: &CycleAppearance, _, cx| {
                crate::app::prefs::cycle_theme(cx);
            }))
            .bg(cx.theme().background)
            .child(Topbar::new(
                self.project_title.clone(),
                this.clone(),
                conn_count,
                connected_count,
            ))
            .child(body)
            .child(StatusBar::new(conn_count, connected_count))
    }
}

fn render_inspector(
    selected: Option<Entity<ConnectionEntry>>,
    cx: &mut Context<Workspace>,
) -> impl IntoElement {
    let border = cx.theme().border;
    let muted = cx.theme().muted_foreground;

    let content = if let Some(ent) = selected {
        let entry = ent.read(cx);
        let engine = entry.config.engine();
        let label = entry.config.label().to_string();
        let state = entry.state.label().to_string();
        let summary = match &entry.state {
            ConnectionState::Failed { reason, .. } => notify::error_one_liner(reason).to_string(),
            ConnectionState::Connected(_) => "Ready for browsing and queries".to_string(),
            ConnectionState::Connecting { .. } => "Opening connection".to_string(),
            ConnectionState::Disconnected => "Click to connect".to_string(),
        };

        v_flex()
            .gap_3()
            .p_3()
            .child(
                v_flex()
                    .gap_1()
                    .child(
                        div()
                            .text_sm()
                            .font_weight(FontWeight::SEMIBOLD)
                            .text_color(cx.theme().foreground)
                            .truncate()
                            .child(label),
                    )
                    .child(
                        div()
                            .text_xs()
                            .text_color(muted)
                            .child(format!("{} connection", engine_name(engine))),
                    ),
            )
            .child(h_flex().gap_2().child(engine_chip(engine, cx)).child(metadata_pill("state", state, cx)))
            .child(
                inspector_section(
                    "Activity",
                    vec![
                        ("Recent", "Schema refresh"),
                        ("Saved", "0 queries"),
                        ("Pinned", "No pinned objects"),
                    ],
                    cx,
                ),
            )
            .child(
                inspector_note("Health", &summary, cx),
            )
            .into_any_element()
    } else {
        v_flex()
            .gap_3()
            .p_3()
            .child(inspector_note(
                "Selection",
                "Choose a connection, table, cell, or query to see details here.",
                cx,
            ))
            .child(inspector_section(
                "Shortcuts",
                vec![
                    ("Command", if cfg!(target_os = "macos") { "⌘K" } else { "Ctrl K" }),
                    ("Run query", if cfg!(target_os = "macos") { "⌘↵" } else { "Ctrl Enter" }),
                    ("Sidebar", if cfg!(target_os = "macos") { "⌘\\" } else { "Ctrl \\" }),
                ],
                cx,
            ))
            .into_any_element()
    };

    v_flex()
        .w(gpui::px(286.0))
        .h_full()
        .flex_shrink_0()
        .border_l_1()
        .border_color(border)
        .bg(cx.theme().background)
        .child(
            h_flex()
                .h(gpui::px(38.0))
                .px_3()
                .items_center()
                .border_b_1()
                .border_color(border.opacity(0.86))
                .child(
                    div()
                        .text_xs()
                        .font_bold()
                        .font_family(cx.theme().mono_font_family.clone())
                        .text_color(muted)
                        .child("INSPECTOR"),
                ),
        )
        .child(content)
}

fn inspector_note(
    title: &'static str,
    body: &str,
    cx: &mut Context<Workspace>,
) -> impl IntoElement {
    v_flex()
        .gap_1()
        .p_2()
        .rounded(gpui::px(7.0))
        .border_1()
        .border_color(cx.theme().border.opacity(0.82))
        .bg(cx.theme().muted.opacity(0.28))
        .child(
            div()
                .text_xs()
                .font_weight(FontWeight::SEMIBOLD)
                .text_color(cx.theme().foreground)
                .child(title),
        )
        .child(
            div()
                .text_xs()
                .text_color(cx.theme().muted_foreground)
                .child(body.to_string()),
        )
}

fn inspector_section(
    title: &'static str,
    rows: Vec<(&'static str, &'static str)>,
    cx: &mut Context<Workspace>,
) -> impl IntoElement {
    v_flex()
        .gap_2()
        .child(
            div()
                .text_xs()
                .font_bold()
                .font_family(cx.theme().mono_font_family.clone())
                .text_color(cx.theme().muted_foreground)
                .child(title),
        )
        .children(rows.into_iter().map(|(label, value)| {
            h_flex()
                .h(gpui::px(24.0))
                .items_center()
                .justify_between()
                .border_b_1()
                .border_color(cx.theme().border.opacity(0.42))
                .child(
                    div()
                        .text_xs()
                        .text_color(cx.theme().muted_foreground)
                        .child(label),
                )
                .child(
                    div()
                        .text_xs()
                        .text_color(cx.theme().foreground.opacity(0.88))
                        .truncate()
                        .child(value),
                )
        }))
}

fn connection_state_dot(state: &ConnectionState, t: &gpui_component::Theme) -> gpui::Hsla {
    match state {
        ConnectionState::Disconnected => t.muted_foreground.opacity(0.75),
        ConnectionState::Connecting { .. } => t.yellow.opacity(0.95),
        ConnectionState::Connected(_) => t.green_light,
        ConnectionState::Failed { .. } => t.red,
    }
}
